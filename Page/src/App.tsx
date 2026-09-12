import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ComponentProps,
  type MouseEvent,
} from 'react'
import { App as KonstaApp, Button, Segmented, SegmentedButton } from 'konsta/react'
import { LiquidGlass } from '@sohumsuthar/liquid-glass'
import { MarkGithubIcon } from '@primer/octicons-react'
import {
  ArrowRight,
  BadgeCheck,
  Boxes,
  ChevronRight,
  Code2,
  Download,
  Gauge,
  GitFork,
  Languages,
  Menu,
  Moon,
  Music2,
  Orbit,
  ShieldCheck,
  Sparkles,
  Sun,
  Waves,
  X,
} from 'lucide-react'
import { HashRouter, Link, useLocation } from 'react-router-dom'
import appIcon from '../../resources/icon-dark.ico'
import settingsIcon from '../../resources/in_app/settings/settings.png'
import widgetIcon from '../../resources/in_app/settings/widget.png'
import allByMyDesignCover from './assets/all-by-my-design.jpg'
import { GlassFilter } from './components/GlassFilter'
import { ProductDemo } from './components/ProductDemo'
import {
  copy,
  DOC_KEYS,
  DOWNLOAD_NIGHTLY,
  DOWNLOAD_RELEASE,
  GITHUB_URL,
  localePath,
  type DemoMode,
  type DocKey,
  type Locale,
} from './content'

const DocumentPage = lazy(() => import('./components/DocumentPage'))
type SiteTheme = 'light' | 'dark'

const THEME_STORAGE_KEY = 'winisland-site-theme'

const systemTheme = (): SiteTheme =>
  window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'

const storedTheme = (): SiteTheme | null => {
  try {
    const value = window.localStorage.getItem(THEME_STORAGE_KEY)
    return value === 'light' || value === 'dark' ? value : null
  } catch {
    return null
  }
}

function useSiteTheme() {
  const [preference, setPreference] = useState<SiteTheme | null>(storedTheme)
  const [system, setSystem] = useState<SiteTheme>(systemTheme)
  const theme = preference ?? system

  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const update = (event: MediaQueryListEvent) => setSystem(event.matches ? 'dark' : 'light')
    media.addEventListener('change', update)
    return () => media.removeEventListener('change', update)
  }, [])

  useLayoutEffect(() => {
    document.documentElement.dataset.theme = theme
    document.documentElement.style.colorScheme = theme
    document.querySelector('meta[name="theme-color"]')?.setAttribute(
      'content',
      theme === 'dark' ? '#0f1013' : '#f5f5f7',
    )
  }, [theme])

  const toggle = useCallback(() => {
    const next = theme === 'dark' ? 'light' : 'dark'
    setPreference(next)
    try {
      window.localStorage.setItem(THEME_STORAGE_KEY, next)
    } catch {}
  }, [theme])

  return { theme, toggle }
}

const DOWNLOAD_PROBE_TIMEOUT = 2500
const DOWNLOAD_PROBES = {
  mirror: 'https://winisland.cn/WinIsland/probe.bin',
  github: 'https://raw.githubusercontent.com/WinIslandProject/WinIsland/master/README.md',
} as const
const DOWNLOAD_MIRRORS: Record<string, string> = {
  [DOWNLOAD_RELEASE]: 'https://winisland.cn/WinIsland/download/stable',
  [DOWNLOAD_NIGHTLY]: 'https://winisland.cn/WinIsland/download/nightly',
}

const measureDownloadSource = (url: string) =>
  new Promise<number>((resolve) => {
    const controller = new AbortController()
    const started = performance.now()
    let settled = false
    let timeout = 0

    const finish = (duration: number) => {
      if (settled) return
      settled = true
      window.clearTimeout(timeout)
      resolve(duration)
    }

    timeout = window.setTimeout(() => {
      controller.abort()
      finish(Infinity)
    }, DOWNLOAD_PROBE_TIMEOUT)

    const separator = url.includes('?') ? '&' : '?'
    fetch(`${url}${separator}probe=${Date.now()}`, {
      cache: 'no-store',
      mode: 'no-cors',
      signal: controller.signal,
    }).then(
      () => finish(performance.now() - started),
      () => finish(Infinity),
    )
  })

const resolveDownloadUrl = async (source: string) => {
  const mirror = DOWNLOAD_MIRRORS[source]
  if (!mirror) return source

  const [mirrorDuration, githubDuration] = await Promise.all([
    measureDownloadSource(DOWNLOAD_PROBES.mirror),
    measureDownloadSource(DOWNLOAD_PROBES.github),
  ])

  return Number.isFinite(mirrorDuration) && mirrorDuration <= githubDuration ? mirror : source
}

type RoutedDownloadButtonProps = Omit<ComponentProps<typeof Button>, 'href' | 'onClick'> & {
  href: string
}

function RoutedDownloadButton({ href, ...props }: RoutedDownloadButtonProps) {
  const routing = useRef(false)

  const handleClick = useCallback(
    (event: MouseEvent<HTMLButtonElement>) => {
      if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return

      event.preventDefault()
      if (routing.current) return
      routing.current = true

      void resolveDownloadUrl(href)
        .then((target) => window.location.assign(target))
        .finally(() => {
          routing.current = false
        })
    },
    [href],
  )

  return <Button {...props} href={href} onClick={handleClick} />
}

const pageFromPath = (pathname: string) => {
  const cleanPath = pathname.replace(/^\/zh(?=\/|$)/, '') || '/'
  if (cleanPath === '/') return 'home' as const
  const key = cleanPath.replace(/^\//, '') as DocKey
  return DOC_KEYS.includes(key) ? key : 'not-found'
}

function App() {
  return (
    <HashRouter>
      <Site />
    </HashRouter>
  )
}

function Site() {
  const location = useLocation()
  const { theme, toggle: toggleTheme } = useSiteTheme()
  const locale: Locale = location.pathname.startsWith('/zh') ? 'zh' : 'en'
  const page = pageFromPath(location.pathname)

  useEffect(() => {
    window.scrollTo({ top: 0, behavior: 'instant' })
  }, [page])

  useEffect(() => {
    document.documentElement.lang = locale === 'zh' ? 'zh-CN' : 'en'
    document.title = 'WinIsland'
  }, [locale])

  return (
    <KonstaApp theme="ios" dark={theme === 'dark'} safeAreas={false} className="site-app">
      <GlassFilter />
      <SiteHeader locale={locale} theme={theme} onThemeToggle={toggleTheme} />
      <main>
        <div className="page-transition" key={page}>
          {page === 'home' && <HomePage locale={locale} />}
          {page === 'download' && <DownloadPage locale={locale} />}
          {page !== 'home' && page !== 'download' && page !== 'not-found' && (
            <Suspense fallback={<div className="docs-loading page-top section-shell" aria-busy="true"><span /></div>}>
              <DocumentPage locale={locale} page={page} />
            </Suspense>
          )}
          {page === 'not-found' && <NotFound locale={locale} />}
        </div>
      </main>
      <SiteFooter locale={locale} />
    </KonstaApp>
  )
}

function SiteHeader({
  locale,
  theme,
  onThemeToggle,
}: {
  locale: Locale
  theme: SiteTheme
  onThemeToggle: () => void
}) {
  const [open, setOpen] = useState(false)
  const [indicator, setIndicator] = useState({ x: 0, width: 0, visible: false })
  const headerRef = useRef<HTMLElement>(null)
  const menuButtonRef = useRef<HTMLButtonElement>(null)
  const navRef = useRef<HTMLElement>(null)
  const navLinks = useRef<Array<HTMLAnchorElement | null>>([])
  const location = useLocation()
  const text = copy[locale]
  const otherLocale: Locale = locale === 'en' ? 'zh' : 'en'
  const currentPath = location.pathname.replace(/^\/zh(?=\/|$)/, '') || '/'
  const guidePaths = ['/guide', '/getting-started', '/plugin-dev', '/api-changelog']
  const isGuidePath = guidePaths.includes(currentPath) || currentPath.startsWith('/plugin-dev/')
  const activeIndex = currentPath === '/' ? 0 : isGuidePath ? 1 : currentPath === '/changelog' ? 2 : -1
  const themeLabel = theme === 'dark'
    ? locale === 'zh' ? '切换为浅色模式' : 'Switch to light mode'
    : locale === 'zh' ? '切换为深色模式' : 'Switch to dark mode'

  const moveIndicator = useCallback((index: number) => {
    const nav = navRef.current
    const link = navLinks.current[index]
    if (!nav || !link) {
      setIndicator((value) => ({ ...value, visible: false }))
      return
    }

    const navRect = nav.getBoundingClientRect()
    const linkRect = link.getBoundingClientRect()
    setIndicator({ x: linkRect.left - navRect.left, width: linkRect.width, visible: true })
  }, [])

  useEffect(() => setOpen(false), [location.pathname])

  useEffect(() => {
    if (!open) return

    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setOpen(false)
        menuButtonRef.current?.focus()
      }
    }
    const closeOnOutsidePress = (event: PointerEvent) => {
      if (!headerRef.current?.contains(event.target as Node)) setOpen(false)
    }

    document.addEventListener('keydown', closeOnEscape)
    document.addEventListener('pointerdown', closeOnOutsidePress)
    const focusFrame = window.requestAnimationFrame(() => {
      if (window.matchMedia('(max-width: 820px)').matches) navLinks.current[0]?.focus()
    })
    return () => {
      window.cancelAnimationFrame(focusFrame)
      document.removeEventListener('keydown', closeOnEscape)
      document.removeEventListener('pointerdown', closeOnOutsidePress)
    }
  }, [open])

  useLayoutEffect(() => {
    const frame = window.requestAnimationFrame(() => moveIndicator(activeIndex))
    const sync = () => moveIndicator(activeIndex)
    window.addEventListener('resize', sync)
    return () => {
      window.cancelAnimationFrame(frame)
      window.removeEventListener('resize', sync)
    }
  }, [activeIndex, locale, moveIndicator])

  return (
    <header ref={headerRef} className="site-header">
      <LiquidGlass macro className="nav-glass" contentClassName="nav-inner">
        <Link className="brand" to={localePath(locale)} aria-label="WinIsland home">
          <img src={appIcon} alt="" />
          <span>WinIsland</span>
        </Link>

        <nav
          id="primary-navigation"
          ref={navRef}
          className={open ? 'primary-nav is-open' : 'primary-nav'}
          aria-label={locale === 'zh' ? '主要导航' : 'Primary navigation'}
          onMouseLeave={() => moveIndicator(activeIndex)}
        >
          <span
            className="nav-indicator"
            aria-hidden="true"
            style={{
              opacity: indicator.visible ? 1 : 0,
              width: indicator.width,
              transform: `translate3d(${indicator.x}px, 0, 0)`,
            }}
          />
          <Link
            ref={(node) => { navLinks.current[0] = node }}
            className={activeIndex === 0 ? 'is-current' : ''}
            aria-current={activeIndex === 0 ? 'page' : undefined}
            to={localePath(locale)}
            onMouseEnter={() => moveIndicator(0)}
          >
            {text.nav.product}
          </Link>
          <Link
            ref={(node) => { navLinks.current[1] = node }}
            className={activeIndex === 1 ? 'is-current' : ''}
            aria-current={activeIndex === 1 ? 'page' : undefined}
            to={localePath(locale, '/guide')}
            onMouseEnter={() => moveIndicator(1)}
          >
            {text.nav.guide}
          </Link>
          <Link
            ref={(node) => { navLinks.current[2] = node }}
            className={activeIndex === 2 ? 'is-current' : ''}
            aria-current={activeIndex === 2 ? 'page' : undefined}
            to={localePath(locale, '/changelog')}
            onMouseEnter={() => moveIndicator(2)}
          >
            {text.nav.changelog}
          </Link>
          <a
            ref={(node) => { navLinks.current[3] = node }}
            className="external-nav-link"
            href={GITHUB_URL}
            target="_blank"
            rel="noreferrer"
            aria-label="GitHub (opens in a new tab)"
            onMouseEnter={() => moveIndicator(3)}
          >
            <MarkGithubIcon size={16} />
            <span>GitHub</span>
          </a>
          <Link
            className={currentPath === '/download' ? 'mobile-nav-download is-current' : 'mobile-nav-download'}
            aria-current={currentPath === '/download' ? 'page' : undefined}
            to={localePath(locale, '/download')}
          >
            <Download size={16} />
            <span>{text.nav.download}</span>
          </Link>
        </nav>

        <div className="nav-actions">
          <button
            className="theme-toggle"
            type="button"
            aria-label={themeLabel}
            title={themeLabel}
            onClick={onThemeToggle}
          >
            {theme === 'dark' ? <Sun size={17} /> : <Moon size={17} />}
          </button>
          <Link
            className="language-link"
            to={localePath(otherLocale, currentPath)}
            aria-label={locale === 'en' ? '切换为中文' : 'Switch to English'}
          >
            <Languages size={16} />
            <span>{text.language}</span>
          </Link>
          <Link
            className={currentPath === '/download' ? 'nav-download is-current' : 'nav-download'}
            aria-current={currentPath === '/download' ? 'page' : undefined}
            to={localePath(locale, '/download')}
          >
            {text.nav.download}
          </Link>
          <button
            ref={menuButtonRef}
            className="menu-button"
            type="button"
            aria-label={open
              ? locale === 'zh' ? '关闭导航菜单' : 'Close navigation menu'
              : locale === 'zh' ? '打开导航菜单' : 'Open navigation menu'}
            aria-controls="primary-navigation"
            aria-expanded={open}
            onClick={() => setOpen((value) => !value)}
          >
            {open ? <X size={19} /> : <Menu size={19} />}
          </button>
        </div>
      </LiquidGlass>
    </header>
  )
}

function HomePage({ locale }: { locale: Locale }) {
  const text = copy[locale]
  const [demoMode, setDemoMode] = useState<DemoMode>('media')

  const scrollToMoments = () => {
    document.getElementById('moments')?.scrollIntoView({ behavior: 'smooth' })
  }

  return (
    <>
      <section className="hero section-shell">
        <div className="hero-aura hero-aura--blue" aria-hidden="true" />
        <div className="hero-aura hero-aura--violet" aria-hidden="true" />
        <div className="hero-copy">
          <h1 className={locale === 'zh' ? 'hero-title--zh' : undefined}>
            {locale === 'zh'
              ? text.hero.title.split('\n').map((line) => <span key={line}>{line}</span>)
              : text.hero.title}
          </h1>
          <p className="hero-body">{text.hero.body}</p>
          <div className="hero-actions">
            <Link
              className="apple-button apple-button--primary apple-link-button"
              to={localePath(locale, '/download')}
            >
              <Download size={18} />
              {text.hero.primary}
            </Link>
            <Button
              className="apple-button apple-button--secondary"
              large
              rounded
              tonal
              onClick={scrollToMoments}
            >
              {text.hero.secondary}
              <ChevronRight size={17} />
            </Button>
          </div>
        </div>

        <ProductDemo locale={locale} variant="hero" />
      </section>

      <section className="moments-section" id="moments">
        <div className="section-heading section-shell">
          <h2>{text.moments.title}</h2>
          <p>{text.moments.body}</p>
        </div>
        <div className="moments-stage section-shell">
          <Segmented strong rounded className="mode-selector" aria-label="Demo mode">
            {(Object.keys(text.moments.modes) as DemoMode[]).map((mode) => (
              <SegmentedButton
                key={mode}
                active={demoMode === mode}
                onClick={() => setDemoMode(mode)}
              >
                {text.moments.modes[mode]}
              </SegmentedButton>
            ))}
          </Segmented>
          <div className="moments-canvas">
            <ProductDemo locale={locale} mode={demoMode} />
          </div>
        </div>
      </section>

      <section className="features-section section-shell">
        <div className="section-heading section-heading--center">
          <h2>{text.features.title}</h2>
        </div>
        <div className="feature-grid">
          {text.features.items.map((feature, index) => {
            const Icon = [Music2, Waves, Sparkles][index]
            return (
              <LiquidGlass
                key={feature.title}
                className={`feature-card feature-card--${feature.accent}`}
                contentClassName="feature-card-content"
              >
                <div className="feature-visual" aria-hidden="true">
                  {index === 0 && (
                    <div className="mini-player">
                      <img className="mini-cover" src={allByMyDesignCover} alt="" />
                      <span className="mini-lyric">{text.demo.lyric}</span>
                      <span className="mini-wave"><i /><i /><i /><i /><i /><i /></span>
                    </div>
                  )}
                  {index === 1 && (
                    <div className="motion-orbits">
                      <span /><span /><span />
                      <Orbit />
                    </div>
                  )}
                  {index === 2 && (
                    <div className="widget-icons">
                      <img src={widgetIcon} alt="" />
                      <img src={settingsIcon} alt="" />
                    </div>
                  )}
                </div>
                <span className="feature-icon"><Icon size={20} /></span>
                <h3>{feature.title}</h3>
                <p>{feature.body}</p>
              </LiquidGlass>
            )
          })}
        </div>
      </section>

      <section className="tech-section section-shell">
        <div className="tech-heading">
          <h2>{text.performance.title}</h2>
          <p>{text.performance.body}</p>
        </div>
        <LiquidGlass macro className="tech-strip" contentClassName="tech-strip-content">
          {text.performance.items.map((item, index) => {
            const Icon = [ShieldCheck, Gauge, Boxes][index]
            return (
              <div className="tech-item" key={item.value}>
                <span><Icon size={19} /></span>
                <div><small>{item.label}</small><strong>{item.value}</strong></div>
                <p>{item.body}</p>
              </div>
            )
          })}
        </LiquidGlass>
      </section>

      <section className="open-section section-shell">
        <div>
          <h2>{text.open.title}</h2>
          <p>{text.open.body}</p>
        </div>
        <div className="open-actions">
          <Button className="apple-button apple-button--dark" rounded large href={GITHUB_URL}>
            <GitFork size={18} />{text.open.source}
          </Button>
          <Link
            className="apple-button apple-button--secondary apple-link-button"
            to={localePath(locale, '/plugin-dev')}
          >
            <Code2 size={18} />{text.open.plugins}
          </Link>
        </div>
      </section>

      <DownloadSection locale={locale} />
    </>
  )
}

function DownloadSection({ locale }: { locale: Locale }) {
  const text = copy[locale].download
  return (
    <section className="download-section">
      <div className="section-shell download-inner">
        <h2>{text.title}</h2>
        <p>{text.body}</p>
        <div className="download-actions">
          <RoutedDownloadButton className="apple-button apple-button--white" rounded large href={DOWNLOAD_RELEASE}>
            <Download size={18} />{text.stable}
          </RoutedDownloadButton>
          <Link to={localePath(locale, '/download')}>{text.nightly}<ArrowRight size={16} /></Link>
        </div>
      </div>
    </section>
  )
}

function DownloadPage({ locale }: { locale: Locale }) {
  const text = copy[locale].download
  return (
    <div className="download-page page-top section-shell">
      <div className="page-hero page-hero--download">
        <h1>{text.title}</h1>
        <p>{text.body}</p>
      </div>
      <div className="channel-grid">
        <LiquidGlass macro className="channel-card" contentClassName="channel-card-content">
          <span className="channel-label"><BadgeCheck size={17} />Stable</span>
          <h2>{text.stable}</h2>
          <p>{locale === 'zh' ? '适合大多数用户的稳定版本。' : 'The stable build recommended for most people.'}</p>
          <RoutedDownloadButton className="apple-button apple-button--primary" rounded large href={DOWNLOAD_RELEASE}>
            <Download size={18} />{text.stable}
          </RoutedDownloadButton>
        </LiquidGlass>
        <LiquidGlass macro className="channel-card" contentClassName="channel-card-content">
          <span className="channel-label"><Sparkles size={17} />Nightly</span>
          <h2>{text.nightly}</h2>
          <p>{text.nightlyHint}</p>
          <RoutedDownloadButton className="apple-button apple-button--secondary" rounded large tonal href={DOWNLOAD_NIGHTLY}>
            <Download size={18} />{text.nightly}
          </RoutedDownloadButton>
        </LiquidGlass>
      </div>
      <div className="requirements-panel">
        <div><span>01</span><strong>{locale === 'zh' ? '下载安装程序' : 'Download the installer'}</strong></div>
        <div><span>02</span><strong>{locale === 'zh' ? '允许身份注册提示' : 'Approve identity registration'}</strong></div>
      </div>
      <p className="requirements-note">
        {locale === 'zh'
          ? '系统要求：Windows 10 2004 或更高版本 / Windows 11，64 位架构，支持 Skia 的现代显卡。'
          : 'System requirements: Windows 10 version 2004 or later / Windows 11, 64-bit architecture, and a modern GPU with Skia support.'}
      </p>
    </div>
  )
}

function NotFound({ locale }: { locale: Locale }) {
  const text = copy[locale].notFound
  return (
    <div className="not-found page-top section-shell">
      <span>404</span>
      <h1>{text.title}</h1>
      <p>{text.body}</p>
      <Link className="apple-button apple-button--primary apple-link-button" to={localePath(locale)}>
        {text.action}
      </Link>
    </div>
  )
}

function SiteFooter({ locale }: { locale: Locale }) {
  const text = copy[locale]
  return (
    <footer className="site-footer">
      <div className="section-shell footer-grid">
        <div className="footer-brand">
          <Link className="brand" to={localePath(locale)}>
            <img src={appIcon} alt="" /><span>WinIsland</span>
          </Link>
          <p>{text.footer.license}</p>
        </div>
        <div>
          <strong>{text.footer.product}</strong>
          <Link to={localePath(locale)}>{text.nav.product}</Link>
          <Link to={localePath(locale, '/download')}>{text.nav.download}</Link>
        </div>
        <div>
          <strong>{text.footer.resources}</strong>
          <Link to={localePath(locale, '/guide')}>{text.nav.guide}</Link>
          <Link to={localePath(locale, '/plugin-dev')}>{text.docs.pages['plugin-dev']}</Link>
          <Link to={localePath(locale, '/changelog')}>{text.nav.changelog}</Link>
        </div>
        <div>
          <strong>{text.footer.community}</strong>
          <a href={GITHUB_URL} target="_blank" rel="noreferrer">GitHub</a>
          <span>QQ · 435799156</span>
        </div>
      </div>
      <div className="section-shell footer-bottom">
        <span>© 2026 WinIsland</span>
        <span>Made for Windows, inspired by thoughtful design.</span>
      </div>
    </footer>
  )
}

export default App
