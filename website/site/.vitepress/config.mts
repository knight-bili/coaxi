import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Coaxi',
  base: '/',
  cleanUrls: false,
  lang: 'en-US',
  locales: {
    root: { label: 'English', lang: 'en-US' },
    zh: { label: '简体中文', lang: 'zh-CN' }
  },
  head: [
    ['link', { rel: 'icon', type: 'image/png', href: '/assets/coaxi-icon.png' }],
    ['link', { rel: 'stylesheet', href: '/assets/site.css' }],
    ['meta', { name: 'theme-color', content: '#fbfbfa' }]
  ],
  ...(process.env.COAXI_SITE_ORIGIN ? { sitemap: { hostname: process.env.COAXI_SITE_ORIGIN } } : {}),
  transformHtml(html, id) {
    const path = id.replaceAll('\\', '/')
    const page = path.includes('/docs/') ? 'docs' : path.includes('/download/') ? 'download' : 'home'
    const lang = path.includes('/zh/') ? 'zh' : 'en'
    return html.replace('<body', `<body data-page="${page}" data-lang="${lang}"`)
  }
})
