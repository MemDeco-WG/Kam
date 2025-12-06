/**
 * 插件管理器
 * 提供插件加载、hook系统和API接口
 */
class PluginManager {
  constructor() {
    this.plugins = new Map();
    this.hooks = new Map();
    this.pluginButtons = {
      header: [],
      sidebar: [],
      bottom: []
    };
    this.pluginPages = new Map();
    this.isInitialized = false;
    
    if (window.core.isDebugMode()) {
      window.core.logDebug('PluginManager initialized', 'PLUGIN');
    }
  }

  /**
   * 初始化插件管理器
   */
  async init() {
    if (this.isInitialized) return;
    
    try {
      // 创建插件目录结构
      await this.ensurePluginDirectories();
      
      // 加载已安装的插件
      await this.loadInstalledPlugins();
      
      this.isInitialized = true;
      
      if (window.core.isDebugMode()) {
        window.core.logDebug('PluginManager initialization completed', 'PLUGIN');
      }
    } catch (error) {
      console.error('Failed to initialize PluginManager:', error);
    }
  }

  /**
   * 确保插件目录存在
   */
  async ensurePluginDirectories() {
    // 在实际应用中，这里可能需要检查文件系统
    // 目前只是占位符
    if (window.core.isDebugMode()) {
      window.core.logDebug('Plugin directories ensured', 'PLUGIN');
    }
  }

  /**
   * 加载已安装的插件
   */
  async loadInstalledPlugins() {
    // 扫描plugins目录下的插件配置文件
    try {
      const pluginConfigs = await this.scanPluginConfigs();
      
      for (const config of pluginConfigs) {
        try {
          await this.loadPluginFromConfig(config);
        } catch (error) {
          if (window.core.isDebugMode()) {
            window.core.logDebug(`Failed to load plugin ${config.id}: ${error.message}`, 'PLUGIN');
          }
        }
      }
    } catch (error) {
      if (window.core.isDebugMode()) {
        window.core.logDebug('No plugins found or failed to scan plugins', 'PLUGIN');
      }
    }
  }

  /**
   * 扫描插件配置文件
   */
  async scanPluginConfigs() {
    const configs = [];
    // 在实际环境中，这里应该扫描文件系统
    // 目前返回空数组，表示没有插件
    return configs;
  }

  /**
   * 从配置加载插件
   * @param {Object} config - 插件配置
   */
  async loadPluginFromConfig(config) {
    if (this.plugins.has(config.id)) {
      if (window.core.isDebugMode()) {
        window.core.logDebug(`Plugin ${config.id} already loaded`, 'PLUGIN');
      }
      return;
    }

    try {
      // 根据配置类型加载插件
      let pluginInstance;
      
      if (config.type === 'class') {
        // 动态导入插件类
        const pluginModule = await import(/* @vite-ignore */ `../plugins/${config.id}/${config.entry || 'index.js'}`);
        const PluginClass = pluginModule.default || pluginModule[config.className || config.id];
        
        if (!PluginClass) {
          throw new Error(`Plugin ${config.id} does not export the required class`);
        }
        
        pluginInstance = new PluginClass();
      } else if (config.type === 'function') {
        // 函数式插件
        const pluginModule = await import(/* @vite-ignore */ `../plugins/${config.id}/${config.entry || 'index.js'}`);
        const pluginFunction = pluginModule.default || pluginModule[config.functionName || 'init'];
        
        if (typeof pluginFunction !== 'function') {
          throw new Error(`Plugin ${config.id} does not export the required function`);
        }
        
        pluginInstance = { init: pluginFunction };
      } else {
        throw new Error(`Unsupported plugin type: ${config.type}`);
      }
      
      // 提供插件API
      const pluginAPI = this.createPluginAPI(config.id);
      
      // 存储插件信息
      this.plugins.set(config.id, {
        instance: pluginInstance,
        config: config,
        api: pluginAPI
      });
      
      // 初始化插件
      if (pluginInstance.init) {
        await pluginInstance.init(pluginAPI);
      }
      
      if (window.core.isDebugMode()) {
        window.core.logDebug(`Plugin ${config.id} loaded successfully`, 'PLUGIN');
      }
    } catch (error) {
      console.error(`Failed to load plugin ${config.id}:`, error);
      throw error;
    }
  }

  /**
   * 加载单个插件（保留兼容性）
   * @param {string} pluginId - 插件ID
   */
  async loadPlugin(pluginId) {
    // 尝试加载传统格式的插件配置
    const config = {
      id: pluginId,
      type: 'class',
      entry: 'index.js'
    };
    
    return this.loadPluginFromConfig(config);
  }

  /**
   * 旧版本的loadPlugin方法内容（已废弃）
   */
  async _loadPluginLegacy(pluginId) {
    if (this.plugins.has(pluginId)) {
      if (window.core.isDebugMode()) {
        window.core.logDebug(`Plugin ${pluginId} already loaded`, 'PLUGIN');
      }
      return;
    }

    try {
      // 动态导入插件
      const pluginModule = await import(/* @vite-ignore */ `../plugins/${pluginId}/index.js`);
      const PluginClass = pluginModule.default || pluginModule[pluginId];
      
      if (!PluginClass) {
        throw new Error(`Plugin ${pluginId} does not export a default class`);
      }

      // 创建插件实例
      const pluginInstance = new PluginClass();
      
      // 提供插件API
      const pluginAPI = this.createPluginAPI(pluginId);
      
      // 初始化插件
      if (pluginInstance.init && typeof pluginInstance.init === 'function') {
        await pluginInstance.init(pluginAPI);
      }
      
      // 注册插件
      this.plugins.set(pluginId, {
        instance: pluginInstance,
        api: pluginAPI,
        metadata: pluginInstance.metadata || {}
      });
      
      if (window.core.isDebugMode()) {
        window.core.logDebug(`Plugin ${pluginId} loaded successfully`, 'PLUGIN');
      }
      
      // 触发插件加载事件
      this.triggerHook('plugin:loaded', { pluginId, plugin: pluginInstance });
      
    } catch (error) {
      console.error(`Failed to load plugin ${pluginId}:`, error);
      throw error;
    }
  }

  /**
   * 卸载插件
   * @param {string} pluginId - 插件ID
   */
  async unloadPlugin(pluginId) {
    const plugin = this.plugins.get(pluginId);
    if (!plugin) return;

    try {
      // 调用插件的清理方法
      if (plugin.instance.cleanup && typeof plugin.instance.cleanup === 'function') {
        await plugin.instance.cleanup();
      }
      
      // 移除插件按钮
      this.removePluginButtons(pluginId);
      
      // 移除插件页面
      this.removePluginPages(pluginId);
      
      // 移除插件hooks
      this.removePluginHooks(pluginId);
      
      // 从插件列表中移除
      this.plugins.delete(pluginId);
      
      if (window.core.isDebugMode()) {
        window.core.logDebug(`Plugin ${pluginId} unloaded`, 'PLUGIN');
      }
      
      // 触发插件卸载事件
      this.triggerHook('plugin:unloaded', { pluginId });
      
    } catch (error) {
      console.error(`Failed to unload plugin ${pluginId}:`, error);
    }
  }

  /**
   * 创建插件API
   * @param {string} pluginId - 插件ID
   * @returns {object} 插件API对象
   */
  createPluginAPI(pluginId) {
    return {
      // 按钮管理
      addButton: (location, button) => this.addPluginButton(pluginId, location, button),
      
      // 页面管理
      addPage: (pageId, pageConfig) => this.addPluginPage(pluginId, pageId, pageConfig),
      
      // Hook系统
      addHook: (hookName, callback) => this.addHook(hookName, callback, pluginId),
      
      // 设置管理
      getSetting: (key, defaultValue) => this.getPluginSetting(pluginId, key, defaultValue),
      setSetting: (key, value) => this.setPluginSetting(pluginId, key, value),
      
      // 工具方法
      showToast: window.core.showToast.bind(window.core),
      showDialog: {
        confirm: window.DialogManager.showConfirm.bind(window.DialogManager),
        input: window.DialogManager.showInput.bind(window.DialogManager),
        popup: window.DialogManager.showPopup.bind(window.DialogManager)
      }
    };
  }

  /**
   * 添加Hook
   * @param {string} hookName - Hook名称
   * @param {function} callback - 回调函数
   * @param {string} pluginId - 插件ID
   */
  addHook(hookName, callback, pluginId) {
    if (!this.hooks.has(hookName)) {
      this.hooks.set(hookName, []);
    }
    
    this.hooks.get(hookName).push({
      callback,
      pluginId
    });
    
    if (window.core.isDebugMode()) {
      window.core.logDebug(`Hook ${hookName} added by plugin ${pluginId}`, 'PLUGIN');
    }
  }

  /**
   * 移除Hook
   * @param {string} hookName - Hook名称
   * @param {function} callback - 回调函数
   * @param {string} pluginId - 插件ID
   */
  removeHook(hookName, callback, pluginId) {
    if (!this.hooks.has(hookName)) return;
    
    const hooks = this.hooks.get(hookName);
    const index = hooks.findIndex(hook => hook.callback === callback && hook.pluginId === pluginId);
    
    if (index !== -1) {
      hooks.splice(index, 1);
      if (hooks.length === 0) {
        this.hooks.delete(hookName);
      }
    }
  }

  /**
   * 触发Hook
   * @param {string} hookName - Hook名称
   * @param {*} data - 传递给Hook的数据
   */
  async triggerHook(hookName, data = {}) {
    if (!this.hooks.has(hookName)) return;
    
    const hooks = this.hooks.get(hookName);
    const results = [];
    
    for (const hook of hooks) {
      try {
        const result = await hook.callback(data);
        results.push(result);
      } catch (error) {
        console.error(`Hook ${hookName} error in plugin ${hook.pluginId}:`, error);
      }
    }
    
    return results;
  }

  /**
   * 添加插件按钮
   * @param {string} pluginId - 插件ID
   * @param {string} location - 按钮位置 (header/sidebar/bottom)
   * @param {object} button - 按钮配置
   */
  addPluginButton(pluginId, location, button) {
    if (!this.pluginButtons[location]) {
      console.error(`Invalid button location: ${location}`);
      return;
    }
    
    const buttonConfig = {
      ...button,
      pluginId,
      id: button.id || `plugin-${pluginId}-${Date.now()}`
    };
    
    this.pluginButtons[location].push(buttonConfig);
    
    // 刷新按钮显示
    this.refreshButtons();
    
    if (window.core.isDebugMode()) {
      window.core.logDebug(`Button added to ${location} by plugin ${pluginId}`, 'PLUGIN');
    }
  }

  /**
   * 移除插件按钮
   * @param {string} pluginId - 插件ID
   * @param {string} location - 按钮位置
   * @param {string} buttonId - 按钮ID
   */
  removePluginButton(pluginId, location, buttonId) {
    if (!this.pluginButtons[location]) return;
    
    const index = this.pluginButtons[location].findIndex(
      btn => btn.pluginId === pluginId && btn.id === buttonId
    );
    
    if (index !== -1) {
      this.pluginButtons[location].splice(index, 1);
      this.refreshButtons();
    }
  }

  /**
   * 移除插件的所有按钮
   * @param {string} pluginId - 插件ID
   */
  removePluginButtons(pluginId) {
    for (const location in this.pluginButtons) {
      this.pluginButtons[location] = this.pluginButtons[location].filter(
        btn => btn.pluginId !== pluginId
      );
    }
    this.refreshButtons();
  }

  /**
   * 刷新按钮显示
   */
  refreshButtons() {
    // 刷新顶栏按钮
    const headerContainer = document.getElementById('plugin-header-actions');
    if (headerContainer) {
      headerContainer.innerHTML = '';
      this.pluginButtons.header.forEach(button => {
        const btn = this.createButtonElement(button);
        headerContainer.appendChild(btn);
      });
    }
    
    // 刷新侧栏按钮
    const sidebarContainer = document.getElementById('plugin-sidebar-actions');
    if (sidebarContainer) {
      sidebarContainer.innerHTML = '';
      this.pluginButtons.sidebar.forEach(button => {
        const btn = this.createButtonElement(button, 'sidebar');
        sidebarContainer.appendChild(btn);
      });
    }
    
    // 刷新底栏按钮
    const bottomContainer = document.getElementById('plugin-bottom-actions');
    if (bottomContainer) {
      bottomContainer.innerHTML = '';
      this.pluginButtons.bottom.forEach(button => {
        const btn = this.createButtonElement(button, 'bottom');
        bottomContainer.appendChild(btn);
      });
    }
  }

  /**
   * 创建按钮元素
   * @param {object} button - 按钮配置
   * @param {string} type - 按钮类型
   * @returns {HTMLElement} 按钮元素
   */
  createButtonElement(button, type = 'header') {
    const btn = document.createElement('button');
    btn.className = `plugin-${type}-btn`;
    btn.id = button.id;
    btn.title = button.title || '';
    btn.innerHTML = `<span class="material-symbols-rounded">${button.icon}</span>`;
    
    if (button.text && type === 'bottom') {
      btn.innerHTML += `<span class="btn-text">${button.text}</span>`;
    }
    
    if (button.action && typeof button.action === 'function') {
      btn.addEventListener('click', button.action);
    }
    
    return btn;
  }

  /**
   * 添加插件页面
   * @param {string} pluginId - 插件ID
   * @param {string} pageId - 页面ID
   * @param {object} pageConfig - 页面配置
   */
  addPluginPage(pluginId, pageId, pageConfig) {
    const fullPageId = `plugin-${pluginId}-${pageId}`;
    
    this.pluginPages.set(fullPageId, {
      ...pageConfig,
      pluginId,
      originalId: pageId
    });
    
    // 添加到应用的页面配置中
    if (window.app && window.app.pagesConfig) {
      window.app.pagesConfig[fullPageId] = {
        title: pageConfig.title,
        icon: pageConfig.icon,
        file: `plugin-${pluginId}-${pageId}`,
        isPlugin: true,
        pluginId
      };
    }
    
    if (window.core.isDebugMode()) {
      window.core.logDebug(`Page ${fullPageId} added by plugin ${pluginId}`, 'PLUGIN');
    }
  }

  /**
   * 移除插件页面
   * @param {string} pluginId - 插件ID
   * @param {string} pageId - 页面ID
   */
  removePluginPage(pluginId, pageId) {
    const fullPageId = `plugin-${pluginId}-${pageId}`;
    
    this.pluginPages.delete(fullPageId);
    
    // 从应用的页面配置中移除
    if (window.app && window.app.pagesConfig) {
      delete window.app.pagesConfig[fullPageId];
    }
  }

  /**
   * 移除插件的所有页面
   * @param {string} pluginId - 插件ID
   */
  removePluginPages(pluginId) {
    const pagesToRemove = [];
    
    for (const [pageId, pageConfig] of this.pluginPages) {
      if (pageConfig.pluginId === pluginId) {
        pagesToRemove.push(pageId);
      }
    }
    
    pagesToRemove.forEach(pageId => {
      this.pluginPages.delete(pageId);
      if (window.app && window.app.pagesConfig) {
        delete window.app.pagesConfig[pageId];
      }
    });
  }

  /**
   * 移除插件的所有hooks
   * @param {string} pluginId - 插件ID
   */
  removePluginHooks(pluginId) {
    for (const [hookName, hooks] of this.hooks) {
      const filteredHooks = hooks.filter(hook => hook.pluginId !== pluginId);
      if (filteredHooks.length === 0) {
        this.hooks.delete(hookName);
      } else {
        this.hooks.set(hookName, filteredHooks);
      }
    }
  }

  /**
   * 获取插件设置
   * @param {string} pluginId - 插件ID
   * @param {string} key - 设置键
   * @param {*} defaultValue - 默认值
   * @returns {*} 设置值
   */
  getPluginSetting(pluginId, key, defaultValue = null) {
    try {
      const settings = JSON.parse(localStorage.getItem(`plugin_${pluginId}_settings`) || '{}');
      return settings[key] !== undefined ? settings[key] : defaultValue;
    } catch (error) {
      console.error(`Failed to get plugin setting ${key} for ${pluginId}:`, error);
      return defaultValue;
    }
  }

  /**
   * 设置插件设置
   * @param {string} pluginId - 插件ID
   * @param {string} key - 设置键
   * @param {*} value - 设置值
   */
  setPluginSetting(pluginId, key, value) {
    try {
      const settings = JSON.parse(localStorage.getItem(`plugin_${pluginId}_settings`) || '{}');
      settings[key] = value;
      localStorage.setItem(`plugin_${pluginId}_settings`, JSON.stringify(settings));
    } catch (error) {
      console.error(`Failed to set plugin setting ${key} for ${pluginId}:`, error);
    }
  }

  /**
   * 获取已加载的插件列表
   * @returns {Array} 插件列表
   */
  getLoadedPlugins() {
    return Array.from(this.plugins.keys());
  }

  /**
   * 获取插件信息
   * @param {string} pluginId - 插件ID
   * @returns {object|null} 插件信息
   */
  getPluginInfo(pluginId) {
    const plugin = this.plugins.get(pluginId);
    return plugin ? {
      id: pluginId,
      metadata: plugin.metadata,
      loaded: true
    } : null;
  }


}

// 创建全局插件管理器实例
const pluginManager = new PluginManager();
window.PluginManager = pluginManager;

export default PluginManager;