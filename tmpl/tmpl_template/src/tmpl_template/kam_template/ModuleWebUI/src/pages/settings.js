class SettingsPage {
  constructor() {
    this.eventHandlers = new Map();
    this.settingsConfig = null;
    this.currentSettings = {};
    this.hasChanges = false;
  }

  render() {
    return `
                <div class="settings-form" id="settings-form">
                    <!-- 设置项将在这里动态生成 -->
                </div>
        `;
  }

  async onShow() {
    await this.loadSettings();
  }

  getPageActions() {
    return [
      {
        icon: "refresh",
        title: window.i18n.t("settings.refresh"),
        action: () => this.refreshSettings(),
      },
      {
        icon: "restart_alt",
        title: window.i18n.t("settings.reset"),
        action: () => this.resetSettings(),
      },
      {
        icon: "save",
        title: window.i18n.t("settings.save"),
        action: () => this.saveSettings(),
      },
    ];
  }

  async refreshSettings() {
    await this.loadSettings();
    this.hasChanges = false;
  }

  cleanup() {
    this.eventHandlers.forEach(({ element, event, handler }) => {
      element.removeEventListener(event, handler);
    });
    this.eventHandlers.clear();
  }

  async loadSettings() {
    try {
      // 通过cat命令读取settings.json配置文件
      await this.loadSettingsConfig();

      // 加载当前设置值
      await this.loadCurrentSettings();

      // 渲染设置界面
      this.renderSettings();
    } catch (error) {
      window.core.showError("加载设置失败", error.message);
      this.renderErrorState();
    }
  }

  async loadSettingsConfig() {
    return new Promise((resolve, reject) => {
      if (!window.core.isKSUEnvironment()) {
        // 浏览器环境使用动态导入作为fallback
        import("../../settings.json")
          .then(module => {
            this.settingsConfig = module.default;
            resolve();
          })
          .catch(reject);
        return;
      }

      // KSU环境使用cat命令读取配置文件
      const settingsPath = `${window.core.MODULE_PATH}/settings.json`;
      window.core.execCommand(`cat "${settingsPath}"`, (output, success) => {
        if (success && output.trim()) {
          try {
            this.settingsConfig = JSON.parse(output.trim());
            if (window.core.isDebugMode()) {
              window.core.logDebug('Settings config loaded via cat', 'SETTINGS');
            }
            resolve();
          } catch (parseError) {
            window.core.logDebug(`Failed to parse settings.json: ${parseError.message}`, 'SETTINGS');
            reject(new Error(`配置文件格式错误: ${parseError.message}`));
          }
        } else {
          window.core.logDebug('Failed to read settings.json via cat', 'SETTINGS');
          reject(new Error('无法读取配置文件'));
        }
      });
    });
  }

  async loadCurrentSettings() {
    if (!window.core.isKSUEnvironment()) {
      // 浏览器环境使用localStorage
      const saved = localStorage.getItem("modulewebui_settings");
      this.currentSettings = saved
        ? JSON.parse(saved)
        : this.getDefaultSettings();
      return;
    }

    // KSU环境中读取sh配置文件
    const settingsFile = `${window.core.MODULE_PATH}/config.sh`;

    return new Promise((resolve) => {
      window.core.execCommand(
        `cat "${settingsFile}" 2>/dev/null || echo ""`,
        (output, success) => {
          try {
            this.currentSettings =
              this.parseShConfig(output) || this.getDefaultSettings();
          } catch (error) {
            this.currentSettings = this.getDefaultSettings();
          }
          resolve();
        }
      );
    });
  }

  parseShConfig(content) {
    const settings = {};
    if (!content) return settings;

    content.split("\n").forEach((line) => {
      // 移除行首尾空白
      line = line.trim();
      
      // 跳过空行和注释行
      if (!line || line.startsWith('#')) {
        return;
      }
      
      // 匹配变量赋值，支持等号前后的空格
      const match = line.match(/^([A-Z_][A-Z0-9_]*)\s*=\s*(.*)$/);
      if (match) {
        const [, key, value] = match;
        
        // 处理值，支持各种引号格式
        let parsedValue = value;
        
        // 移除行尾注释（但保留引号内的#）
        if (!value.match(/^["']/)) {
          // 如果不是以引号开始，移除#后的内容
          parsedValue = value.replace(/\s*#.*$/, '');
        } else {
          // 如果以引号开始，需要找到匹配的结束引号
          const quoteChar = value.charAt(0);
          let endQuoteIndex = -1;
          let escaped = false;
          
          for (let i = 1; i < value.length; i++) {
            if (escaped) {
              escaped = false;
              continue;
            }
            if (value.charAt(i) === '\\') {
              escaped = true;
              continue;
            }
            if (value.charAt(i) === quoteChar) {
              endQuoteIndex = i;
              break;
            }
          }
          
          if (endQuoteIndex > 0) {
            parsedValue = value.substring(0, endQuoteIndex + 1);
          }
        }
        
        // 移除外层引号并处理转义字符
        if ((parsedValue.startsWith('"') && parsedValue.endsWith('"')) ||
            (parsedValue.startsWith("'") && parsedValue.endsWith("'"))) {
          parsedValue = parsedValue.slice(1, -1);
          // 处理转义字符
          parsedValue = parsedValue.replace(/\\(.)/g, '$1');
        }
        
        settings[key] = parsedValue;
        
        if (window.core.isDebugMode()) {
          window.core.logDebug(`Parsed setting: ${key}=${parsedValue}`, 'SETTINGS');
        }
      }
    });
    
    return settings;
  }

  getDefaultSettings() {
    const defaults = {};
    Object.entries(this.settingsConfig.settings).forEach(([key, config]) => {
      defaults[key] = config.default;
    });
    return defaults;
  }

  renderSettings() {
    const formEl = document.getElementById("settings-form");
    if (!formEl) {
      console.error("Settings form container not found");
      window.core.showToast(window.i18n.t("settings.loadFailed"), "error");
      return;
    }

    const settingsHtml = Object.entries(this.settingsConfig.settings)
      .map(([key, config]) => {
        return this.renderSetting(key, config);
      })
      .join("");

    formEl.innerHTML = settingsHtml;

    // 绑定事件
    this.bindEvents();
  }

  renderSetting(key, config) {
    const currentValue = this.currentSettings[key] ?? config.default;
    
    // 获取国际化的标题和描述
    const title = this.getSettingTitle(key, config);
    const description = this.getSettingDescription(key, config);
    
    // 创建带有国际化支持的配置对象
    const localizedConfig = {
      ...config,
      title: title,
      description: description
    };

    switch (config.type) {
      case "boolean":
        return this.renderBooleanSetting(key, localizedConfig, currentValue);
      case "select":
        return this.renderSelectSetting(key, localizedConfig, currentValue);
      case "input":
        return this.renderInputSetting(key, localizedConfig, currentValue);
      default:
        return "";
    }
  }

  renderBooleanSetting(key, config, currentValue) {
    const checked = currentValue === true || currentValue === "true";
    return `
            <fieldset class="switches">
                <label>
                    <span>${config.title}</span>
                    <input type="checkbox" data-key="${key}" ${
      checked ? "checked" : ""
    }>
                </label>
            </fieldset>
        `;
  }

  renderSelectSetting(key, config, currentValue) {
    const optionsHtml = config.choices
      .map((choice) => {
        // 获取选项标签（支持JSON内嵌翻译）
        let label = choice.label;
        
        // 优先使用JSON配置中的翻译
        if (choice.translations) {
          const currentLang = window.i18n.getCurrentLanguage() || 'zh';
          const translation = choice.translations[currentLang];
          if (translation) {
            label = translation;
          }
        }
        // 其次尝试从国际化文件获取
        else if (choice.i18n) {
          const i18nLabel = window.i18n.t(choice.i18n);
          if (i18nLabel && i18nLabel !== choice.i18n) {
            label = i18nLabel;
          }
        }
        
        return `<option value="${choice.value}" ${
          choice.value === currentValue ? "selected" : ""
        }>${label}</option>`;
      })
      .join("");

    return `
            <label>
                <span>${config.title}</span>
                <select data-key="${key}">
                    ${optionsHtml}
                </select>
            </label>
        `;
  }

  renderInputSetting(key, config, currentValue) {
    return `
            <label>
                <span>${config.title}</span>
                <input type="text" data-key="${key}" value="${currentValue}" placeholder="${config.default}">
            </label>
        `;
  }

  bindEvents() {
    // 开关
    document
      .querySelectorAll('input[type="checkbox"][data-key]')
      .forEach((checkbox) => {
        const handler = () =>
          this.updateSetting(checkbox.dataset.key, checkbox.checked);
        checkbox.addEventListener("change", handler);
        this.eventHandlers.set(`checkbox-${checkbox.dataset.key}`, {
          element: checkbox,
          event: "change",
          handler,
        });
      });

    // 选择框
    document.querySelectorAll("select[data-key]").forEach((select) => {
      const handler = () =>
        this.updateSetting(select.dataset.key, select.value);
      select.addEventListener("change", handler);
      this.eventHandlers.set(`select-${select.dataset.key}`, {
        element: select,
        event: "change",
        handler,
      });
    });

    // 文本输入框
    document
      .querySelectorAll('input[type="text"][data-key]')
      .forEach((input) => {
        const handler = () =>
          this.updateSetting(input.dataset.key, input.value);
        input.addEventListener("input", handler);
        this.eventHandlers.set(`input-${input.dataset.key}`, {
          element: input,
          event: "input",
          handler,
        });
      });
  }

  updateSetting(key, value) {
    this.currentSettings[key] = value;
    this.hasChanges = true;
    window.core.logDebug(`设置更新: ${key} = ${value}`, "SETTINGS");
  }

  resetChanges() {
    this.hasChanges = false;
  }

  async saveSettings() {
    try {
      if (!window.core.isKSUEnvironment()) {
        // 浏览器环境保存到localStorage
        localStorage.setItem(
          "modulewebui_settings",
          JSON.stringify(this.currentSettings)
        );
        this.markSaved();
        window.core.showToast("设置已保存", "success");
        return;
      }

      // KSU环境保存到sh文件
      const settingsFile = `${window.core.MODULE_PATH}/config.sh`;
      const shContent = this.generateShConfig();

      window.core.execCommand(
        `echo '${shContent}' > "${settingsFile}"`,
        (output, success) => {
          if (success) {
            this.markSaved();
            window.core.showToast(
              window.i18n.t("settings.saveSuccess"),
              "success"
            );
          } else {
            window.core.showError(window.i18n.t("settings.saveFailed"), output);
          }
        }
      );
    } catch (error) {
      window.core.showError("保存设置失败", error.message);
    }
  }

  generateShConfig() {
    const lines = [
      "#!/bin/bash",
      "",
      "#  Configuration",
      "# Generated automatically - do not edit manually",
      ""
    ];
    
    // 按设置配置的顺序生成，并添加注释
    if (this.settingsConfig && this.settingsConfig.settings) {
      Object.entries(this.settingsConfig.settings).forEach(([key, config]) => {
        const value = this.currentSettings[key] ?? config.default;
        
        // 添加设置项注释
        const title = this.getSettingTitle(key, config);
        const description = this.getSettingDescription(key, config);
        
        lines.push(`# ${title}`);
        if (description && description !== title) {
          lines.push(`# ${description}`);
        }
        
        // 处理值的引号
        let quotedValue;
        if (typeof value === 'string' && (value.includes(' ') || value.includes('#') || value.includes('"') || value.includes("'"))) {
          // 需要引号的情况：包含空格、注释符或引号
          quotedValue = `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
        } else {
          quotedValue = `"${value}"`;
        }
        
        lines.push(`${key}=${quotedValue}`);
        lines.push(""); // 空行分隔
      });
    } else {
      // 如果没有配置信息，使用简单格式
      Object.entries(this.currentSettings).forEach(([key, value]) => {
        const quotedValue = `"${value.toString().replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
        lines.push(`${key}=${quotedValue}`);
      });
    }
    
    return lines.join("\n");
  }
  
  /**
   * 获取设置项标题（支持JSON内嵌翻译）
   */
  getSettingTitle(key, config) {
    // 优先使用JSON配置中的翻译
    if (config.translations) {
      const currentLang = window.i18n.getCurrentLanguage() || 'zh';
      const translation = config.translations[currentLang];
      if (translation && translation.title) {
        return translation.title;
      }
    }
    
    // 其次尝试从国际化文件获取
    if (config.i18n && config.i18n.title) {
      const i18nTitle = window.i18n.t(config.i18n.title);
      if (i18nTitle && i18nTitle !== config.i18n.title) {
        return i18nTitle;
      }
    }
    
    // 最后使用配置中的默认标题
    return config.title || key;
  }
  
  /**
   * 获取设置项描述（支持JSON内嵌翻译）
   */
  getSettingDescription(key, config) {
    // 优先使用JSON配置中的翻译
    if (config.translations) {
      const currentLang = window.i18n.getCurrentLanguage() || 'zh';
      const translation = config.translations[currentLang];
      if (translation && translation.description) {
        return translation.description;
      }
    }
    
    // 其次尝试从国际化文件获取
    if (config.i18n && config.i18n.description) {
      const i18nDescription = window.i18n.t(config.i18n.description);
      if (i18nDescription && i18nDescription !== config.i18n.description) {
        return i18nDescription;
      }
    }
    
    // 最后使用配置中的默认描述
    return config.description || '';
  }

  markSaved() {
    this.hasChanges = false;
  }

  async resetSettings() {
    const confirmed = await window.app.showDialog.confirm(
      window.i18n.t("settings.reset"),
      window.i18n.t("settings.resetConfirm")
    );

    if (confirmed) {
      this.currentSettings = this.getDefaultSettings();

      // 保存重置后的设置
      if (!window.core.isKSUEnvironment()) {
        localStorage.setItem(
          "modulewebui_settings",
          JSON.stringify(this.currentSettings)
        );
      } else {
        const settingsFile = `${window.core.MODULE_PATH}/config.sh`;
        const shContent = this.generateShConfig();
        window.core.execCommand(`echo '${shContent}' > "${settingsFile}"`);
      }

      this.hasChanges = false;
      this.renderSettings();
      window.core.showToast(window.i18n.t("settings.resetSuccess"), "success");
    }
  }

  renderErrorState() {
    const sectionsEl = document.getElementById("settings-sections");
    const loadingEl = document.getElementById("settings-loading");

    loadingEl.style.display = "none";
    sectionsEl.style.display = "block";
    sectionsEl.innerHTML = `
            <div class="error-state">
                <span class="material-symbols-rounded">error</span>
                <span>加载设置失败</span>
                <button class="retry-btn" onclick="this.loadSettings()">
                    <span class="material-symbols-rounded">refresh</span>
                    重试
                </button>
            </div>
        `;
  }
}

export { SettingsPage };
