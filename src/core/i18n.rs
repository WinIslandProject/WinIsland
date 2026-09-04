use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, LazyLock, RwLock};
use windows::Win32::Globalization::GetUserDefaultLocaleName;

#[derive(Clone, Debug)]
pub struct Language {
    pub code: String,
    pub name: String,
}

pub struct I18n {
    pub current_lang: String,
    translations: HashMap<String, String>,
    pub available_languages: Vec<Language>,
    lang_files: HashMap<String, String>,
    plugin_translation_bundles: BTreeMap<u64, PluginTranslationBundle>,
}

struct PluginTranslationBundle {
    language: String,
    translations: HashMap<String, String>,
}

type EmbeddedLang = (&'static str, &'static str);

static I18N: LazyLock<Arc<RwLock<I18n>>> = LazyLock::new(|| {
    let i18n = I18n::new();
    Arc::new(RwLock::new(i18n))
});

fn embedded_langs() -> &'static [EmbeddedLang] {
    &[
        (
            "en_us.lang",
            include_str!("../../resources/in_app/lang/en_us.lang"),
        ),
        (
            "zh_cn.lang",
            include_str!("../../resources/in_app/lang/zh_cn.lang"),
        ),
        (
            "es_es.lang",
            include_str!("../../resources/in_app/lang/es_es.lang"),
        ),
    ]
}

fn lang_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("resources")
        .join("in_app")
        .join("lang")
}

fn parse_lang_name(content: &str) -> Option<String> {
    content
        .lines()
        .take_while(|line| line.starts_with('!'))
        .find_map(|line| line.strip_prefix("!lang_name="))
        .map(|name| name.trim().to_owned())
}

fn parse_translations(content: &str) -> HashMap<String, String> {
    content
        .lines()
        .filter_map(|line| {
            (!line.starts_with('!'))
                .then(|| line.split_once('='))
                .flatten()
        })
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .collect()
}

fn format_args(template: &str, args: &[&str]) -> String {
    args.iter()
        .enumerate()
        .fold(template.to_owned(), |result, (index, value)| {
            result
                .replacen("{}", value, 1)
                .replace(&format!("{{{index}}}"), value)
        })
}

fn discover_disk_langs() -> (Vec<Language>, HashMap<String, String>) {
    let mut languages = Vec::new();
    let mut file_map = HashMap::new();
    let dir = lang_dir();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("lang")
                && let Some(filename) = path.file_name().and_then(|n| n.to_str())
                && let Ok(content) = std::fs::read_to_string(&path)
            {
                let code = Path::new(filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(filename)
                    .to_string();
                let name = parse_lang_name(&content).unwrap_or_else(|| code.clone());
                file_map.insert(code.clone(), filename.to_string());
                languages.push(Language { code, name });
            }
        }
    }
    (languages, file_map)
}

impl I18n {
    fn new() -> Self {
        let (disk_langs, disk_files) = discover_disk_langs();
        let mut available = disk_langs;
        let file_map = disk_files;

        for (filename, content) in embedded_langs() {
            let code = Path::new(filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(filename)
                .to_string();
            if !available.iter().any(|l| l.code == code) {
                let name = parse_lang_name(content).unwrap_or_else(|| code.clone());
                available.push(Language { code, name });
            }
        }

        let default_lang = available
            .first()
            .map(|l| l.code.clone())
            .unwrap_or_else(|| "en_us".to_string());

        let mut i18n = I18n {
            current_lang: default_lang.clone(),
            translations: HashMap::new(),
            available_languages: available,
            lang_files: file_map,
            plugin_translation_bundles: BTreeMap::new(),
        };
        i18n.load(&default_lang);
        i18n
    }

    fn load_file_content(&self, lang: &str) -> Option<String> {
        if let Some(filename) = self.lang_files.get(lang) {
            let path = lang_dir().join(filename);
            if let Ok(content) = std::fs::read_to_string(&path) {
                return Some(content);
            }
        }
        for (filename, content) in embedded_langs() {
            let code = Path::new(filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if code == lang {
                return Some(content.to_string());
            }
        }
        None
    }

    pub fn load(&mut self, lang: &str) {
        if let Some(content) = self.load_file_content(lang) {
            self.current_lang = lang.to_string();
            self.translations = parse_translations(&content);
        }
    }

    pub fn get(&self, key: &str) -> String {
        if let Some(value) = self
            .plugin_translation_bundles
            .values()
            .rev()
            .filter(|bundle| bundle.language == self.current_lang)
            .find_map(|bundle| bundle.translations.get(key))
        {
            return value.clone();
        }
        self.translations
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    pub fn register_plugin_translation_bundle(
        &mut self,
        id: u64,
        language: String,
        pairs: Vec<(String, String)>,
    ) {
        self.plugin_translation_bundles.insert(
            id,
            PluginTranslationBundle {
                language,
                translations: pairs.into_iter().collect(),
            },
        );
    }

    pub fn release_plugin_translation_bundle(&mut self, id: u64) {
        self.plugin_translation_bundles.remove(&id);
    }
}

pub fn init_i18n(config_lang: &str) {
    let target_lang = if config_lang == "auto" {
        get_system_lang()
    } else {
        config_lang.to_string()
    };
    I18N.write().unwrap().load(&target_lang);
}

pub fn set_lang(lang: &str) {
    I18N.write().unwrap().load(lang);
}

pub fn current_lang() -> String {
    I18N.read().unwrap().current_lang.clone()
}

pub fn tr(key: &str) -> String {
    I18N.read().unwrap().get(key)
}

pub fn tr_args(key: &str, args: &[&str]) -> String {
    let template = I18N.read().unwrap().get(key);
    format_args(&template, args)
}

pub fn available_langs() -> Vec<Language> {
    I18N.read().unwrap().available_languages.clone()
}

pub fn register_plugin_translation_bundle(
    id: u64,
    language: String,
    pairs: Vec<(String, String)>,
) -> Result<(), &'static str> {
    let mut i18n = I18N.write().map_err(|_| "i18n lock is poisoned")?;
    i18n.register_plugin_translation_bundle(id, language, pairs);
    Ok(())
}

pub fn release_plugin_translation_bundle(id: u64) -> Result<(), &'static str> {
    let mut i18n = I18N.write().map_err(|_| "i18n lock is poisoned")?;
    i18n.release_plugin_translation_bundle(id);
    Ok(())
}

fn get_system_lang() -> String {
    let mut buffer = [0u16; 128];
    // SAFETY: GetUserDefaultLocaleName reads the system locale into the provided
    // buffer. The buffer is stack-allocated with 128 elements, sufficient for any
    // valid locale name. from_utf16_lossy handles potentially malformed input.
    unsafe {
        let len = GetUserDefaultLocaleName(&mut buffer);
        if len > 0 {
            let s = String::from_utf16_lossy(&buffer[..len as usize - 1]);
            let lower = s.to_lowercase();
            if lower.starts_with("zh") {
                return "zh_cn".to_string();
            }
            if lower.starts_with("es") {
                return "es_es".to_string();
            }
            return "en_us".to_string();
        }
    }
    "en_us".to_string()
}
