/// A borrowed translation key-value pair.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TranslationPairV2 {
    pub key: super::Utf8Slice,
    pub value: super::Utf8Slice,
}
