export type Locale = 'en' | 'zh'
export type DemoMode = 'media' | 'notification' | 'widgets'
export type DocKey =
  | 'guide'
  | 'getting-started'
  | 'download'
  | 'plugin-dev'
  | 'plugin-dev/quickstart'
  | 'plugin-dev/abi-lifecycle'
  | 'plugin-dev/services'
  | 'plugin-dev/packaging'
  | 'api-changelog'
  | 'changelog'

export const DOC_KEYS: DocKey[] = [
  'guide',
  'getting-started',
  'download',
  'plugin-dev',
  'plugin-dev/quickstart',
  'plugin-dev/abi-lifecycle',
  'plugin-dev/services',
  'plugin-dev/packaging',
  'api-changelog',
  'changelog',
]

export const localePath = (locale: Locale, path = '/') => {
  const cleanPath = path.startsWith('/') ? path : `/${path}`
  if (locale === 'zh') return cleanPath === '/' ? '/zh' : `/zh${cleanPath}`
  return cleanPath
}

export const copy = {
  en: {
    language: '中文',
    nav: {
      product: 'Product',
      guide: 'Guide',
      changelog: 'Changelog',
      download: 'Download',
    },
    hero: {
      title: 'A small island\nA better desktop',
      body: 'Media, notifications, widgets, and the controls you need — always close, never in the way.',
      primary: 'Download for Windows',
      secondary: 'See how it works',
    },
    demo: {
      track: 'All by My Design',
      artist: 'Monster Siren Records · Nina Storey',
      lyric: 'Bring it all online',
      notificationApp: 'Outlook',
      notification: 'Meeting starts in 10 minutes',
      notificationBody: 'Design review · 3:30 PM',
      month: 'JUL',
      weekday: 'FRIDAY',
    },
    moments: {
      title: 'It changes with what matters now',
      body: 'WinIsland stays compact until there is something worth seeing. Click once to reveal the details and controls you need.',
      modes: {
        media: 'Media',
        notification: 'Alerts',
        widgets: 'Widgets',
      },
    },
    features: {
      title: 'Made to feel like it belongs',
      items: [
        {
          title: 'Your music, in motion',
          body: 'Album art, playback controls, live lyrics, and an audio spectrum come together in one fluid view.',
          accent: 'coral',
        },
        {
          title: 'Spring, not snap',
          body: 'Physics-based transitions make every expansion and return feel responsive without calling attention to themselves.',
          accent: 'blue',
        },
        {
          title: 'Yours to arrange',
          body: 'Choose the widgets and appearance that fit your desktop. WinIsland adapts instead of taking over.',
          accent: 'green',
        },
      ],
    },
    performance: {
      title: 'Native at every layer',
      body: 'A compact Windows experience, built from a small set of fast, dependable parts.',
      items: [
        { label: 'Core', value: 'Rust', body: 'Memory-safe and lightweight' },
        { label: 'Rendering', value: 'Skia', body: 'Hardware-accelerated motion' },
        { label: 'Platform', value: 'Windows', body: 'Native media and system APIs' },
      ],
    },
    open: {
      title: 'Make the island yours',
      body: 'Explore the source, help shape the plugin system, or build something the community has not imagined yet.',
      source: 'View on GitHub',
      plugins: 'Plugin guide',
    },
    download: {
      title: 'Bring WinIsland to your desktop',
      body: 'For Windows 10 version 2004 or later and Windows 11. 64-bit only.',
      stable: 'Download latest release',
      nightly: 'Get nightly build',
      nightlyHint: 'Nightly builds include the newest changes and may be less stable.',
    },
    footer: {
      product: 'Product',
      resources: 'Resources',
      community: 'Community',
      license: 'Released under the GNU GPL v3 License.',
    },
    docs: {
      title: 'WinIsland Guide',
      subtitle: 'Everything you need to install, use, and extend WinIsland.',
      onThisPage: 'Documentation',
      pages: {
        guide: 'What is WinIsland?',
        'getting-started': 'Getting started',
        download: 'Download',
        'plugin-dev': 'Plugin development',
        'plugin-dev/quickstart': 'Plugin quickstart',
        'plugin-dev/abi-lifecycle': 'ABI and lifecycle',
        'plugin-dev/services': 'Host services',
        'plugin-dev/packaging': 'Packaging and installation',
        'api-changelog': 'API changelog',
        changelog: 'Changelog',
      },
    },
    notFound: {
      title: 'This island is empty',
      body: 'The page you are looking for has moved or does not exist.',
      action: 'Return home',
    },
  },
  zh: {
    language: 'EN',
    nav: {
      product: '产品',
      guide: '指南',
      changelog: '更新日志',
      download: '下载',
    },
    hero: {
      title: '小小一座岛，\n让桌面更顺手',
      body: '媒体、通知、小组件和常用控制都近在眼前，需要时出现，其余时间安静隐身。',
      primary: '下载 Windows 版',
      secondary: '看看它如何工作',
    },
    demo: {
      track: 'All by My Design',
      artist: 'Monster Siren Records · Nina Storey',
      lyric: 'Bring it all online',
      notificationApp: 'Outlook',
      notification: '会议将在 10 分钟后开始',
      notificationBody: '设计评审 · 15:30',
      month: '7月',
      weekday: '星期五',
    },
    moments: {
      title: '此刻重要什么，它就呈现什么',
      body: '平时保持紧凑，有值得关注的内容时才轻轻展开。点击一次，就能看到所需信息与控制。',
      modes: {
        media: '媒体',
        notification: '通知',
        widgets: '小组件',
      },
    },
    features: {
      title: '从第一眼起，就像本该如此',
      items: [
        {
          title: '让音乐，跃然岛上',
          body: '专辑封面、播放控制、实时歌词和音频频谱，在一个流畅界面中自然汇合。',
          accent: 'coral',
        },
        {
          title: '有弹性，不突兀',
          body: '基于物理的弹簧动画，让每一次展开与收起都跟手、连贯，又不过分抢眼。',
          accent: 'blue',
        },
        {
          title: '你的桌面，你来安排',
          body: '自由选择小组件与外观。WinIsland 会适应你的桌面，而不是占据它。',
          accent: 'green',
        },
      ],
    },
    performance: {
      title: '每一层，都原生而轻量',
      body: '一组精简而可靠的技术，共同撑起顺滑的 Windows 体验。',
      items: [
        { label: '核心', value: 'Rust', body: '内存安全，保持轻量' },
        { label: '渲染', value: 'Skia', body: '硬件加速，动画顺滑' },
        { label: '平台', value: 'Windows', body: '原生媒体与系统接口' },
      ],
    },
    open: {
      title: '把这座岛，变成你的',
      body: '阅读源码、参与插件系统设计，或做出一个社区从未想过的新功能。',
      source: '在 GitHub 查看',
      plugins: '插件开发指南',
    },
    download: {
      title: '让 WinIsland 登上你的桌面',
      body: '支持 Windows 10 2004 及以上版本与 Windows 11，仅支持 64 位系统。',
      stable: '下载最新正式版',
      nightly: '获取每日预览版',
      nightlyHint: '预览版包含最新改动，但稳定性可能不如正式版。',
    },
    footer: {
      product: '产品',
      resources: '资源',
      community: '社区',
      license: '基于 GNU GPL v3 许可证发布。',
    },
    docs: {
      title: 'WinIsland 指南',
      subtitle: '安装、使用与扩展 WinIsland 所需的一切。',
      onThisPage: '文档目录',
      pages: {
        guide: '什么是 WinIsland？',
        'getting-started': '快速开始',
        download: '下载',
        'plugin-dev': '插件开发',
        'plugin-dev/quickstart': '插件快速开始',
        'plugin-dev/abi-lifecycle': 'ABI 与生命周期',
        'plugin-dev/services': '宿主服务',
        'plugin-dev/packaging': '打包与安装',
        'api-changelog': 'API 更新日志',
        changelog: '更新日志',
      },
    },
    notFound: {
      title: '这座岛上什么也没有',
      body: '你访问的页面已移动或不存在。',
      action: '返回首页',
    },
  },
} as const

export const DOWNLOAD_RELEASE =
  'https://github.com/WinIslandProject/WinIsland/releases/latest/download/WinIsland-Setup.exe'
export const DOWNLOAD_NIGHTLY =
  'https://github.com/WinIslandProject/WinIsland/releases/download/nightly/WinIsland-Nightly-Setup.exe'
export const GITHUB_URL = 'https://github.com/WinIslandProject/WinIsland'
