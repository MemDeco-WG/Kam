/**
 *  主应用程序
 * 提供路由管理、页面加载、设置管理等核心功能
 */
// 导入页面配置
import pagesConfig from "./pages.json" with { type: "json" };
// 导入插件管理器
import PluginManager from "./modules/plugin-manager.js";

class App {
  constructor() {
    this.currentPage = pagesConfig.defaultPage || "home";
    this.pageCache = new Map();
    this.pagesConfig = pagesConfig;
    this.currentPageInstance = null;
    this.pageActions = [];
    this.loadedCSS = new Set(); // 跟踪已加载的CSS文件
    this.pluginManager = new PluginManager();
    this.settings = {
      debugMode: false,
      useNativeToast: window.core.isKSUEnvironment(),
      hue: 300,
      language: "zh",
    };

    this.init();
  }

  // CSS模块动态加载管理
  async loadCSS(href) {
    if (this.loadedCSS.has(href)) {
      return Promise.resolve();
    }

    return new Promise((resolve, reject) => {
      const link = document.createElement('link');
      link.rel = 'stylesheet';
      link.href = href;
      link.onload = () => {
        this.loadedCSS.add(href);
        resolve();
      };
      link.onerror = () => reject(new Error(`Failed to load CSS: ${href}`));
      document.head.appendChild(link);
    });
  }

  async init() {
    try {
      // 加载设置
      await this.loadSettings();

      // 初始化window.i18n
      await window.i18n.init();

      // 设置语言
      if (this.settings.language) {
        await window.i18n.setLanguage(this.settings.language);
      }

      // 初始化插件管理器
      await this.pluginManager.init();

      // 生成导航栏
      this.generateNavigation();

      // 初始化UI
      this.initUI();

      // 触发应用初始化完成hook
      await this.pluginManager.triggerHook('app:initialized', { app: this });

      // 加载默认页面
      await this.loadPage(this.currentPage);

      // 标记应用已加载
      document.body.classList.add("app-loaded");

      if (window.core.isDebugMode()) {
        window.core.logDebug("App initialized successfully", "APP");
        window.core.showToast("[DEBUG] App initialized", "info");
      }
    } catch (error) {
      window.core.showError(error.message, "App initialization failed");
    }
  }

  generateNavigation() {
    const navContent = document.querySelector('.nav-content');
    const sidebarNavItems = document.querySelector('.sidebar-nav-items');
    
    if (!navContent || !sidebarNavItems) return;

    // 清空现有导航
    navContent.innerHTML = '';
    sidebarNavItems.innerHTML = '';

    // 根据配置生成导航项
    const sortedPages = Object.entries(this.pagesConfig.pages)
      .filter(([_, config]) => config.showInNav)
      .sort(([_, a], [__, b]) => (a.order || 999) - (b.order || 999));

    sortedPages.forEach(([pageId, config]) => {
      // 底部导航项
      const navItem = document.createElement('a');
      navItem.href = `#${pageId}`;
      navItem.className = `nav-item ${pageId === this.currentPage ? 'active' : ''}`;
      navItem.dataset.page = pageId;
      navItem.innerHTML = `
        <span class="material-symbols-rounded">${config.icon}</span>
        <span id="nav-${pageId}"></span>
      `;
      navContent.appendChild(navItem);

      // 侧栏导航项
      const sidebarItem = document.createElement('a');
      sidebarItem.href = `#${pageId}`;
      sidebarItem.className = `sidebar-nav-item ${pageId === this.currentPage ? 'active' : ''}`;
      sidebarItem.dataset.page = pageId;
      sidebarItem.innerHTML = `
        <span class="material-symbols-rounded">${config.icon}</span>
        <span id="sidebar-nav-${pageId}"></span>
      `;
      sidebarNavItems.appendChild(sidebarItem);
    });
  }

  initUI() {
    // 导航事件
    this.initNavigation();

    // 设置对话框
    this.initSettings();

    // 更新UI语言
    this.updateUILanguage();

    // 应用主题
    this.applyTheme();
  }

  initNavigation() {
    // 底部导航
    const navItems = document.querySelectorAll(".nav-item");
    navItems.forEach((item) => {
      item.addEventListener("click", (e) => {
        e.preventDefault();
        const page = item.dataset.page;
        this.navigateTo(page);
      });
    });

    // 侧栏导航
    const sidebarItems = document.querySelectorAll(".sidebar-nav-item");
    sidebarItems.forEach((item) => {
      item.addEventListener("click", (e) => {
        e.preventDefault();
        const page = item.dataset.page;
        this.navigateTo(page);
      });
    });

    // 处理浏览器后退/前进
    window.addEventListener("popstate", (e) => {
      const page = e.state?.page || "home";
      this.navigateTo(page, false);
    });
  }

  initSettings() {
    const settingsBtn = document.getElementById("settings-btn");
    const sidebarSettingsBtn = document.getElementById("sidebar-settings-btn");
    const aboutBtn = document.getElementById("about-btn");
    const sidebarAboutBtn = document.getElementById("sidebar-about-btn");
    const settingsDialog = document.getElementById("settings-dialog");
    const settingsCancel = document.getElementById("settings-cancel");
    const settingsSave = document.getElementById("settings-save");

    // 打开设置
    const openSettings = () => {
      this.loadSettingsUI();
      window.DialogManager.showDialogWithAnimation(settingsDialog);
    };

    settingsBtn?.addEventListener("click", openSettings);
    sidebarSettingsBtn?.addEventListener("click", openSettings);

    // 打开关于对话框
    const openAbout = () => {
      this.showAboutDialog();
    };

    aboutBtn?.addEventListener("click", openAbout);
    sidebarAboutBtn?.addEventListener("click", openAbout);

    // 取消设置
    settingsCancel.addEventListener("click", () => {
      window.DialogManager.closeDialogWithAnimation(settingsDialog);
    });
    
    // 点击空白处关闭设置对话框
    settingsDialog.addEventListener("click", (e) => {
      if (e.target === settingsDialog) {
        window.DialogManager.closeDialogWithAnimation(settingsDialog);
      }
    });

    // 保存设置
    settingsSave.addEventListener("click", async () => {
      await this.saveSettings();
      window.DialogManager.closeDialogWithAnimation(settingsDialog);
      window.core.showToast("设置已保存", "success");
    });

    // 色调滑块
    const hueSlider = document.getElementById("hue-slider");
    const hueValue = document.getElementById("hue-value");

    hueSlider.addEventListener("input", (e) => {
      const hue = e.target.value;
      hueValue.textContent = hue;
      document.documentElement.style.setProperty("--hue", hue);
    });
  }

  async navigateTo(page, pushState = true) {
    if (this.currentPage === page) return;

    try {
      // 检查当前页面是否有未保存的更改
      if (this.currentPage === 'settings' && this.currentPageInstance && this.currentPageInstance.hasChanges) {
        const confirmed = await window.DialogManager.showConfirm(
          window.i18n.t('settings.unsavedChanges'),
          window.i18n.t('settings.unsavedMessage')
        );
        if (!confirmed) {
          return;
        }
        // 如果用户确认离开，重置未保存状态
        this.currentPageInstance.resetChanges();
      }

      // 更新导航状态
      this.updateNavigation(page);

      // 加载页面
      await this.loadPage(page);

      // 更新浏览器历史
      if (pushState) {
        history.pushState({ page }, "", `#${page}`);
      }

      this.currentPage = page;

      if (window.core.isDebugMode()) {
        window.core.logDebug(`Navigated to page: ${page}`, "NAV");
      }
    } catch (error) {
      window.core.showError(error.message, `Failed to navigate to ${page}`);
    }
  }

  updateNavigation(page) {
    // 更新底部导航
    document.querySelectorAll(".nav-item").forEach((item) => {
      item.classList.toggle("active", item.dataset.page === page);
    });

    // 更新侧栏导航
    document.querySelectorAll(".sidebar-nav-item").forEach((item) => {
      item.classList.toggle("active", item.dataset.page === page);
    });

    // 更新页面标题
    const pageConfig = this.pagesConfig.pages[page];
    const title = pageConfig ? window.i18n.t(pageConfig.title) : window.i18n.t("app.title");
    document.getElementById("page-title").textContent = title;
  }

  async loadPage(page) {
    const pageContent = document.getElementById("page-content");

    // 触发页面加载前hook
    await this.pluginManager.triggerHook('page:before-load', { page, currentPage: this.currentPage });

    // 添加退出动画
    if (this.currentPageInstance) {
      pageContent.classList.add('page-transition-exit');
      await new Promise(resolve => setTimeout(resolve, 100)); // 等待退出动画完成
      
      // 清理当前页面
      if (this.currentPageInstance.cleanup) {
        this.currentPageInstance.cleanup();
      }
      
      // 触发页面卸载hook
      await this.pluginManager.triggerHook('page:unloaded', { page: this.currentPage, instance: this.currentPageInstance });
    }

    // 显示加载状态
    pageContent.innerHTML = `
      <div class="loading">
        <span class="material-symbols-rounded">hourglass_empty</span>
        <span style="margin-left: 8px;">${window.i18n.t("app.loading")}</span>
      </div>
    `;
    pageContent.className = 'page-loading';

    try {
      // 检查缓存
      let pageInstance = this.pageCache.get(page);
      
      if (!pageInstance) {
        // 动态导入页面模块
        pageInstance = await this.importPage(page);
        if (pageInstance) {
          this.pageCache.set(page, pageInstance);
        }
      }

      if (pageInstance) {
        // 加载页面CSS
        const cssPath = this.getPageCSSPath(page);
        try {
          await this.loadCSS(cssPath);
        } catch (error) {
          // CSS文件不存在时不报错，继续渲染页面
          if (window.core.isDebugMode()) {
            window.core.logDebug(`页面CSS文件不存在: ${cssPath}`, 'APP');
          }
        }
        
        // 渲染页面
        const html = await pageInstance.render();
        pageContent.innerHTML = html;

        // 调用页面显示回调
        if (pageInstance.onShow) {
          await pageInstance.onShow();
        }
        
        // 保存当前页面实例引用
        this.currentPageInstance = pageInstance;
        
        // 添加进入动画
        pageContent.classList.remove('page-loading', 'page-transition-exit');
        pageContent.classList.add('page-transition-enter');
        
        // 触发动画
        requestAnimationFrame(() => {
          pageContent.classList.remove('page-transition-enter');
          pageContent.classList.add('page-loaded');
        });
        
        // 触发页面加载完成hook
        await this.pluginManager.triggerHook('page:loaded', { page, instance: pageInstance });
        
        // 处理页面操作按钮
        if (pageInstance.getPageActions && typeof pageInstance.getPageActions === 'function') {
          const actions = pageInstance.getPageActions();
          this.setPageActions(actions);
        } else {
          // 默认刷新按钮
          this.setPageActions([
            {
              icon: 'refresh',
              title: '刷新页面',
              action: () => {
                if (pageInstance.onRefresh && typeof pageInstance.onRefresh === 'function') {
                  pageInstance.onRefresh();
                } else {
                  this.loadPage(this.currentPage);
                }
              }
            }
          ]);
        }
      } else {
        // 页面不存在
        pageContent.innerHTML = `
          <div class="error-state">
            <span class="material-symbols-rounded">error</span>
            <p>${window.i18n.t("messages.pageNotFound")}</p>
          </div>
        `;
        this.currentPageInstance = null;
        // 清空页面操作按钮
        this.setPageActions([]);
      }
    } catch (error) {
      // 清空页面操作按钮
      this.setPageActions([]);
      
      pageContent.innerHTML = `
        <div class="error-state">
          <span class="material-symbols-rounded">error</span>
          <p>${window.i18n.t("messages.pageLoadFailed")}: ${error.message}</p>
        </div>
      `;
      this.currentPageInstance = null;
      if (window.core.isDebugMode()) {
        window.core.logDebug(`Page load failed: ${error.message}`, "APP");
      }
    }
  }

  async importPage(page) {
    try {
      const pageConfig = this.pagesConfig.pages[page];
      if (!pageConfig) {
        throw new Error(`Page configuration not found for: ${page}`);
      }

      // 检查是否为插件页面
      if (pageConfig.isPlugin) {
        const pluginPage = this.pluginManager.pluginPages.get(page);
        if (pluginPage && pluginPage.pageClass) {
          return new pluginPage.pageClass();
        } else {
          throw new Error(`Plugin page class not found for: ${page}`);
        }
      }

      const moduleName = pageConfig.module || page;
      const module = await import(/* @vite-ignore */ `./pages/${moduleName}.js`);
      const PageClass = module[this.getPageClassName(page)];
      return PageClass ? new PageClass() : null;
    } catch (error) {
      if (window.core.isDebugMode()) {
        window.core.logDebug(`Failed to import page ${page}: ${error.message}`, "APP");
      }
      return null;
    }
  }

  getPageClassName(page) {
    // 将页面名转换为类名 (home -> HomePage, about -> AboutPage)
    return page.charAt(0).toUpperCase() + page.slice(1) + 'Page';
  }

  getPageCSSPath(page) {
    // 检查是否在开发环境
    const isDev = import.meta.env?.DEV || window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1';
    
    if (isDev) {
      // 开发环境使用绝对路径
      return `/src/assets/css/pages/${page}.css`;
    } else {
      // 生产环境使用相对路径
      return `./assets/css/pages/${page}.css`;
    }
  }

  async loadSettings() {
    try {
      // 从localStorage加载设置
      const saved = localStorage.getItem("modulewebui-settings");
      if (saved) {
        this.settings = { ...this.settings, ...JSON.parse(saved) };
      }
    } catch (error) {
      if (window.core.isDebugMode()) {
        window.core.logDebug(`Failed to load settings: ${error.message}`, "SETTINGS");
      }
    }
  }

  async saveSettings() {
    try {
      const oldLanguage = this.settings.language;
      
      // 从UI获取设置
      Object.assign(this.settings, {
        debugMode: document.getElementById("debug-mode")?.checked || false,
        useNativeToast: document.getElementById("native-toast")?.checked || false,
        hue: parseInt(document.getElementById("hue-slider")?.value || 300),
        language: document.getElementById("language-select")?.value || "zh"
      });

      // 如果语言改变了，切换语言并更新UI
      if (oldLanguage !== this.settings.language) {
        await window.i18n.setLanguage(this.settings.language);
        this.updateUILanguage();
        await this.loadPage(this.currentPage);
      }

      // 保存到localStorage并应用主题
      localStorage.setItem("modulewebui-settings", JSON.stringify(this.settings));
      this.applyTheme();

      if (window.core.isDebugMode()) {
        window.core.logDebug("Settings saved successfully", "SETTINGS");
      }
    } catch (error) {
      window.core.showError(error.message, "Failed to save settings");
    }
  }

  loadSettingsUI() {
    const elements = {
      'debug-mode': { prop: 'checked', value: this.settings.debugMode },
      'native-toast': { prop: 'checked', value: this.settings.useNativeToast },
      'hue-slider': { prop: 'value', value: this.settings.hue },
      'hue-value': { prop: 'textContent', value: this.settings.hue },
      'language-select': { prop: 'value', value: this.settings.language }
    };

    Object.entries(elements).forEach(([id, config]) => {
      const element = document.getElementById(id);
      if (element) element[config.prop] = config.value;
    });
  }

  updateUILanguage() {
    // 更新导航文本
    Object.entries(this.pagesConfig.pages).forEach(([pageId, config]) => {
      if (config.showInNav) {
        const navElement = document.getElementById(`nav-${pageId}`);
        const sidebarNavElement = document.getElementById(`sidebar-nav-${pageId}`);
        const text = window.i18n.t(config.title);
        
        if (navElement) navElement.textContent = text;
        if (sidebarNavElement) sidebarNavElement.textContent = text;
      }
    });

    // 更新设置对话框文本
    const translations = {
      'settings-title': 'settings.title',
      'setting-language-label': 'settings.language',
      'setting-debug-label': 'settings.debugMode',
      'setting-debug-desc': 'settings.debugModeDesc',
      'setting-toast-label': 'settings.nativeToast',
      'setting-toast-desc': 'settings.nativeToastDesc',
      'setting-hue-label': 'settings.themeHue',
      'settings-cancel': 'settings.cancel',
      'settings-save': 'settings.save'
    };

    Object.entries(translations).forEach(([id, key]) => {
      const element = document.getElementById(id);
      if (element) element.textContent = window.i18n.t(key);
    });
  }

  applyTheme() {
    document.documentElement.style.setProperty("--hue", this.settings.hue);
  }

  // 获取设置值
  getSetting(key) {
    return this.settings[key];
  }
  
  /**
   * 设置页面操作按钮
   * @param {Array} actions - 按钮配置数组 [{icon, title, action, id}]
   */
  setPageActions(actions = []) {
    this.pageActions = actions;
    this.renderPageActions();
  }
  
  /**
   * 渲染页面操作按钮
   */
  renderPageActions() {
    // 渲染顶栏按钮
    const headerContainer = document.getElementById('page-actions');
    if (headerContainer) {
      headerContainer.innerHTML = '';
      this.pageActions.forEach(action => {
        const button = document.createElement('button');
        button.className = 'page-action-btn';
        button.id = action.id || `page-action-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
        button.title = action.title || '';
        button.innerHTML = `<span class="material-symbols-rounded">${action.icon}</span>`;
        
        if (action.action && typeof action.action === 'function') {
          button.addEventListener('click', action.action);
        }
        
        headerContainer.appendChild(button);
      });
    }
    
    // 渲染侧栏按钮
    const sidebarContainer = document.getElementById('sidebar-page-actions');
    if (sidebarContainer) {
      sidebarContainer.innerHTML = '';
      this.pageActions.forEach(action => {
        const button = document.createElement('button');
        button.className = 'sidebar-page-action-btn';
        button.id = `sidebar-${action.id || `page-action-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`}`;
        button.title = action.title || '';
        button.innerHTML = `<span class="material-symbols-rounded">${action.icon}</span>`;
        
        if (action.action && typeof action.action === 'function') {
          button.addEventListener('click', action.action);
        }
        
        sidebarContainer.appendChild(button);
      });
    }
    
    // 刷新插件按钮
    this.pluginManager.refreshButtons();
  }
  
  showAboutDialog() {
    this.githubLink = "https://github.com/APMMDEVS/Module" + "WebUI";
    const aboutContent = `
      <div class="about-content">
        <div class="about-logo">
          <span class="material-symbols-rounded">widgets</span>
        </div>
        <div class="about-text">
          <h3>Module WebUI</h3>
          <p class="version">v2.0.0</p>
          <p class="description">${window.i18n.t('about.description')}</p>
          <div class="about-links">
            <div class="about-link">
              ${window.i18n.t('about.please')}
              <a href="${this.githubLink}" target="_blank">
                ${window.i18n.t('about.github')}
              </a>
              ${window.i18n.t('about.lookoursrc')}
            </div>
          </div>
        </div>
      </div>
    `;
    
    window.DialogManager.showPopup({
      content: aboutContent,
      closable: true
    });
  }
}

// 等待所有依赖模块加载完成后再创建应用实例
function initializeApp() {
  // 检查所有必需的依赖是否已加载
  if (window.core && window.i18n && window.DialogManager) {
    window.app = new App();
    window.settingsManager = window.app; // 兼容性别名
    
    if (window.core.isDebugMode()) {
      window.core.logDebug('All dependencies loaded, App instance created', 'APP');
    }
  } else {
    // 如果依赖未完全加载，延迟10ms后重试
    console.log('Dependencies not loaded, retrying in 10ms...');
    setTimeout(initializeApp, 10);
  }
}

// 启动应用初始化
initializeApp();

export default App;
