import apiChangelog from '../../crates/winisland-plugin-api/ChangeLog.md?raw'
import downloadEn from '../download.md?raw'
import gettingStartedEn from '../getting-started.md?raw'
import guideEn from '../guide.md?raw'
import pluginEn from '../plugin-dev.md?raw'
import pluginAbiEn from '../plugin-dev/abi-lifecycle.md?raw'
import pluginPackagingEn from '../plugin-dev/packaging.md?raw'
import pluginQuickstartEn from '../plugin-dev/quickstart.md?raw'
import pluginServicesEn from '../plugin-dev/services.md?raw'
import downloadZh from '../zh/download.md?raw'
import gettingStartedZh from '../zh/getting-started.md?raw'
import guideZh from '../zh/guide.md?raw'
import faqZh from '../zh/faq.md?raw'
import pluginZh from '../zh/plugin-dev.md?raw'
import pluginAbiZh from '../zh/plugin-dev/abi-lifecycle.md?raw'
import pluginPackagingZh from '../zh/plugin-dev/packaging.md?raw'
import pluginQuickstartZh from '../zh/plugin-dev/quickstart.md?raw'
import pluginServicesZh from '../zh/plugin-dev/services.md?raw'
import changelogEn from '../../Changelog.md?raw'
import changelogZh from '../../Changelog-zh.md?raw'

export const docs = {
  en: {
    guide: guideEn,
    faq: faqZh,
    'getting-started': gettingStartedEn,
    download: downloadEn,
    'plugin-dev': pluginEn,
    'plugin-dev/quickstart': pluginQuickstartEn,
    'plugin-dev/abi-lifecycle': pluginAbiEn,
    'plugin-dev/services': pluginServicesEn,
    'plugin-dev/packaging': pluginPackagingEn,
    'api-changelog': apiChangelog,
    changelog: changelogEn,
  },
  zh: {
    guide: guideZh,
    faq: faqZh,
    'getting-started': gettingStartedZh,
    download: downloadZh,
    'plugin-dev': pluginZh,
    'plugin-dev/quickstart': pluginQuickstartZh,
    'plugin-dev/abi-lifecycle': pluginAbiZh,
    'plugin-dev/services': pluginServicesZh,
    'plugin-dev/packaging': pluginPackagingZh,
    'api-changelog': apiChangelog,
    changelog: changelogZh,
  },
} as const
