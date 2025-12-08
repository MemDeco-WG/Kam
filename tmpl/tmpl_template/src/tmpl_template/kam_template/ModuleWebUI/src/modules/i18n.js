/**
 * 国际化管理器
 * 支持中文、英文、俄文三种语言
 */

class I18n {
  constructor() {
    this.currentLanguage = "zh"; // 默认中文
    this.translations = new Map();
    this.fallbackLanguage = "en";

    // 从localStorage加载语言设置
    const savedLang = localStorage.getItem("modulewebui-language");
    if (savedLang && ["zh", "en", "ru"].includes(savedLang)) {
      this.currentLanguage = savedLang;
    } else {
      // 自动检测浏览器语言
      this.detectBrowserLanguage();
    }
  }

  /**
   * 检测浏览器语言
   */
  detectBrowserLanguage() {
    const browserLang = navigator.language || navigator.userLanguage;

    if (browserLang.startsWith("zh")) {
      this.currentLanguage = "zh";
    } else if (browserLang.startsWith("ru")) {
      this.currentLanguage = "ru";
    } else {
      this.currentLanguage = "en";
    }
  }

  /**
   * 加载语言文件
   */
  async loadLanguage(lang) {
    if (this.translations.has(lang)) {
      return true; // 已经加载过
    }

    try {
      // 使用Vite的动态导入加载主翻译文件
      const translationModule = await import(/* @vite-ignore */ `../i18n/${lang}.json`);
      const translations = translationModule.default;

      // 加载扩展翻译文件
      const moduleTranslations = await this.loadModuleTranslations(lang);

      // 合并翻译
      const mergedTranslations = this.mergeTranslations(
        translations,
        moduleTranslations
      );

      this.translations.set(lang, mergedTranslations);
      return true;
    } catch (error) {
      console.error(`Error loading language ${lang}:`, error);
      // 如果加载失败，使用默认语言
      if (lang !== this.fallbackLanguage) {
        return await this.loadLanguage(this.fallbackLanguage);
      }
      return false;
    }
  }

  /**
   * 加载模块扩展翻译文件
   */
  async loadModuleTranslations(lang) {
    const moduleTranslations = {};

    try {
      // 使用Vite的动态导入加载modules目录下的翻译文件
      const moduleData = await import(/* @vite-ignore */ `../i18n/modules/${lang}.json`);
      Object.assign(moduleTranslations, moduleData.default);

      if (window.core && window.core.isDebugMode()) {
        window.core.logDebug(
          `Loaded module translations for ${lang}`,
          "I18N"
        );
      }
    } catch (error) {
      // 模块翻译文件不存在时不报错，这是正常情况
      if (window.core && window.core.isDebugMode()) {
        window.core.logDebug(
          `No module translations found for ${lang}`,
          "I18N"
        );
      }
    }

    return moduleTranslations;
  }

  /**
   * 合并翻译对象
   */
  mergeTranslations(base, extensions) {
    const result = { ...base };

    for (const [key, value] of Object.entries(extensions)) {
      if (
        typeof value === "object" &&
        value !== null &&
        !Array.isArray(value)
      ) {
        // 递归合并嵌套对象
        result[key] = this.mergeTranslations(result[key] || {}, value);
      } else {
        // 直接覆盖或添加
        result[key] = value;
      }
    }

    return result;
  }

  /**
   * 初始化i18n
   */
  async init() {
    // 加载当前语言
    await this.loadLanguage(this.currentLanguage);

    // 加载备用语言
    if (this.currentLanguage !== this.fallbackLanguage) {
      await this.loadLanguage(this.fallbackLanguage);
    }

    // 更新页面语言属性
    document.documentElement.lang = this.getLanguageCode();
  }

  /**
   * 获取翻译文本
   * @param {string} key - 翻译键，支持点号分隔的嵌套键
   * @param {object} params - 参数对象，用于替换占位符
   * @returns {string} 翻译后的文本
   */
  t(key, params = {}) {
    const translation =
      this.getTranslation(key, this.currentLanguage) ||
      this.getTranslation(key, this.fallbackLanguage) ||
      key;

    // 替换参数占位符
    return this.interpolate(translation, params);
  }

  /**
   * 获取指定语言的翻译
   */
  getTranslation(key, lang) {
    const translations = this.translations.get(lang);
    if (!translations) return null;

    // 支持嵌套键，如 'pages.home.title'
    const keys = key.split(".");
    let value = translations;

    for (const k of keys) {
      if (value && typeof value === "object" && k in value) {
        value = value[k];
      } else {
        return null;
      }
    }

    return value;
  }

  /**
   * 参数插值
   */
  interpolate(text, params) {
    if (typeof text !== "string") return text;

    return text.replace(/\{\{(\w+)\}\}/g, (match, key) => {
      return params[key] !== undefined ? params[key] : match;
    });
  }

  /**
   * 切换语言
   */
  async setLanguage(lang) {
    if (!["zh", "en", "ru"].includes(lang)) {
      console.warn(`Unsupported language: ${lang}`);
      return false;
    }

    // 如果是相同语言，直接返回
    if (this.currentLanguage === lang) {
      return true;
    }

    // 加载新语言
    const success = await this.loadLanguage(lang);
    if (!success) {
      console.error(`Failed to load language: ${lang}`);
      return false;
    }

    this.currentLanguage = lang;

    // 保存到localStorage
    localStorage.setItem("modulewebui-language", lang);

    // 更新页面语言属性
    document.documentElement.lang = this.getLanguageCode();

    // 触发语言变更事件
    window.dispatchEvent(
      new CustomEvent("languageChanged", {
        detail: { language: lang },
      })
    );

    return true;
  }

  /**
   * 获取当前语言
   */
  getCurrentLanguage() {
    return this.currentLanguage;
  }

  /**
   * 获取语言代码（用于HTML lang属性）
   */
  getLanguageCode() {
    const codes = {
      zh: "zh-CN",
      en: "en-US",
      ru: "ru-RU",
    };
    return codes[this.currentLanguage] || "en-US";
  }

  /**
   * 获取支持的语言列表
   */
  getSupportedLanguages() {
    return [
      { code: "zh", name: "中文", nativeName: "中文" },
      { code: "en", name: "English", nativeName: "English" },
      { code: "ru", name: "Russian", nativeName: "Русский" },
    ];
  }

  /**
   * 格式化数字（考虑本地化）
   */
  formatNumber(number, options = {}) {
    const locale = this.getLanguageCode();
    return new Intl.NumberFormat(locale, options).format(number);
  }

  /**
   * 格式化日期（考虑本地化）
   */
  formatDate(date, options = {}) {
    const locale = this.getLanguageCode();
    return new Intl.DateTimeFormat(locale, options).format(date);
  }
}

// 创建全局实例
const i18n = new I18n();

// 导出
export { i18n, I18n };
export default i18n;

// 全局访问
window.i18n = i18n;
