class HomePage {
  constructor() {
    this.modules = [];
    this.refreshInterval = null;
    this.AllowCustomActions = false;
  }

  async render() {
    return `
                <!-- 模块状态显示区域 -->
                <div class="module-status-container">
                    <div id="module-status-card" class="status-card loading">
                        <div class="loading-spinner"></div>
                        <div class="loading-text">${window.i18n.t(
                          "home.modules.loading"
                        )}</div>
                    </div>
                    <div id="custom-actions-container" class="custom-actions-container"></div>
                </div>
        `;
  }

  /**
   * 获取页面操作按钮配置
   * @returns {Array} 按钮配置数组
   */
  getPageActions() {
    return [
      {
        icon: "refresh",
        title: window.i18n.t("home.modules.refreshStatus"),
        action: () => this.onRefresh(),
      },
    ];
  }

  /**
   * 刷新页面数据
   */
  onRefresh() {
    this.loadModuleStatus();
    window.core.showToast("状态已刷新", "success");
  }

  async onShow() {
    // 确保页面内容已渲染完成
    await new Promise((resolve) => requestAnimationFrame(resolve));

    this.initEventListeners();
    await this.loadModuleStatus();

    // 每30秒刷新一次模块状态
    this.refreshInterval = setInterval(() => {
      this.loadModuleStatus();
    }, 30000);

    // 页面显示完成，触发动画
    const pageContent = document.getElementById("page-content");
    if (pageContent) {
      pageContent.classList.add("page-loaded");
    }
  }

  cleanup() {
    if (this.refreshInterval) {
      clearInterval(this.refreshInterval);
      this.refreshInterval = null;
    }
  }

  initEventListeners() {
    // 模块状态卡片点击事件
    const statusCard = document.getElementById("module-status-card");
    if (statusCard) {
      statusCard.addEventListener("click", () => {
        this.loadModuleStatus();
      });
    }
  }

  async loadModuleStatus() {
    const container = document.getElementById("module-status-card");
    if (!container) return;

    try {
      // 读取主模块状态
      const moduleInfo = await this.readModuleInfo(
        window.core.MODULE_PATH || "."
      );
      this.renderModuleStatus(moduleInfo);
    } catch (error) {
      window.core.logDebug(`模块状态加载失败: ${error.message}`, "HOME");
      container.innerHTML = `
                <div class="status-card-content">
                    <div class="status-icon-container">
                        <div class="status-indicator">
                            <span class="material-symbols-rounded">error</span>
                        </div>
                    </div>
                    <div class="status-info-container">
                        <div class="status-title-row">
                            <span>${window.i18n.t(
                              "home.modules.statusCheckFailed"
                            )}</span>
                        </div>
                        <div class="status-details">
                            <div class="status-detail-row">${window.i18n.t(
                              "home.modules.errorInfo"
                            )}: ${error.message}</div>
                        </div>
                    </div>
                </div>
            `;
      container.className = "status-card module-status-card status-error";
    }

    if (this.AllowCustomActions) {
      this.renderCustomActions();
      this.bindCustomActions();
    }
  }

  async renderCustomActions() {
    const customActionsContainer = document.getElementById("custom-actions-container");
    if (!customActionsContainer) return;
    customActionsContainer.innerHTML = `
                    <div class="custom-actions">
                    <button class="custom-action-btn" id="custom-action-1" title="${window.i18n.t("home.customActions.action1")}">
                        <span class="material-symbols-rounded">extension</span>
                        <span class="action-text">${window.i18n.t("home.customActions.action1")}</span>
                    </button>
                    <button class="custom-action-btn" id="custom-action-2" title="${window.i18n.t("home.customActions.action2")}">
                        <span class="material-symbols-rounded">build</span>
                        <span class="action-text">${window.i18n.t("home.customActions.action2")}</span>
                    </button>
                    <button class="custom-action-btn" id="custom-action-3" title="${window.i18n.t("home.customActions.action3")}">
                        <span class="material-symbols-rounded">tune</span>
                        <span class="action-text">${window.i18n.t("home.customActions.action3")}</span>
                    </button>
                </div>
            `;
  }

  async readModuleInfo(modulePath) {
    try {
      // 检查是否在浏览器环境中
      if (!window.core.isKSUEnvironment()) {
        // 浏览器环境模拟数据
        const statuses = ["running", "stopped", "error", "normal-exit"];
        const randomStatus =
          statuses[Math.floor(Math.random() * statuses.length)];

        return {
          name: "Main System Module",
          status: randomStatus,
          path: modulePath,
          pid:
            randomStatus === "running"
              ? Math.floor(Math.random() * 10000) + 1000
              : null,
          startTime:
            randomStatus === "running"
              ? new Date(Date.now() - Math.random() * 86400000).toISOString()
              : null,
          lastUpdate: new Date().toISOString(),
        };
      }

      // KSU环境中读取实际状态文件
      const statusFile = `${modulePath}/info.txt`;

      return new Promise((resolve) => {
        window.core.execCommand(`cat "${statusFile}"`, (output, success) => {
          if (success && output) {
            const info = {
              name: "",
              path: modulePath,
              lastUpdate: new Date().toISOString(),
            };
            output.split("\n").forEach((line) => {
              const [key, value] = line.split("=");
              if (key && value) {
                info[key.trim()] = value.trim();
              }
            });
            resolve(info);
          } else {
            // 如果文件不存在，返回默认状态
            resolve({
              name: "Main System Module",
              status: "unknown",
              path: modulePath,
              pid: null,
              startTime: null,
              lastUpdate: new Date().toISOString(),
            });
          }
        });
      });
    } catch (error) {
      throw new Error(`无法读取模块信息: ${error.message}`);
    }
  }

  renderModuleStatus(moduleInfo) {
    const container = document.getElementById("module-status-card");
    if (!container) return;

    const statusConfig = {
      running: {
        icon: "play_circle",
        title: window.i18n.t("home.modules.status.running"),
        className: "status-running",
      },
      stopped: {
        icon: "stop_circle",
        title: window.i18n.t("home.modules.status.stopped"),
        className: "status-stopped",
      },
      error: {
        icon: "error",
        title: window.i18n.t("home.modules.status.error"),
        className: "status-error",
      },
      "normal-exit": {
        icon: "check_circle",
        title: window.i18n.t("home.modules.status.normal-exit"),
        className: "status-normal-exit",
      },
      unknown: {
        icon: "help",
        title: window.i18n.t("home.modules.status.unknown"),
        className: "status-error",
      },
    };

    const config = statusConfig[moduleInfo.status] || statusConfig["error"];

    container.innerHTML = `
                <div class="status-card-content">
                    <div class="status-icon-container">
                        <div class="status-indicator">
                            <span class="material-symbols-rounded">${
                              config.icon
                            }</span>
                        </div>
                    </div>
                    <div class="status-info-container">
                        <div class="status-title-row">
                            <span>${config.title}</span>
                        </div>
                        <div class="status-details">
                            ${
                              moduleInfo.pid
                                ? `<div class="status-detail-row">${window.i18n.t(
                                    "home.modules.pid"
                                  )}: ${moduleInfo.pid}</div>`
                                : ""
                            }
                            ${
                              moduleInfo.startTime
                                ? `<div class="status-detail-row">${window.i18n.t(
                                    "home.modules.startTime"
                                  )}: ${new Date(
                                    moduleInfo.startTime
                                  ).toLocaleString()}</div>`
                                : ""
                            }
                            <div class="status-detail-row">${window.i18n.t(
                              "home.modules.lastUpdate"
                            )}: ${new Date(
      moduleInfo.lastUpdate
    ).toLocaleString()}</div>
                        </div>
                    </div>
                </div>
        `;
    container.className = `status-card module-status-card ${config.className}`;
  }

  /**
   * 绑定自定义按钮事件
   */
  bindCustomActions() {
    const action1 = document.getElementById('custom-action-1');
    const action2 = document.getElementById('custom-action-2');
    const action3 = document.getElementById('custom-action-3');

    if (action1) {
      action1.addEventListener('click', () => {
        // 开发者可以在这里绑定自定义事件
        window.core.showToast(window.i18n.t("home.customActions.action1Clicked"), "info");
        if (window.core.isDebugMode()) {
          window.core.logDebug('Custom Action 1 clicked', 'HOME');
        }
      });
    }

    if (action2) {
      action2.addEventListener('click', () => {
        // 开发者可以在这里绑定自定义事件
        window.core.showToast(window.i18n.t("home.customActions.action2Clicked"), "info");
        if (window.core.isDebugMode()) {
          window.core.logDebug('Custom Action 2 clicked', 'HOME');
        }
      });
    }

    if (action3) {
      action3.addEventListener('click', () => {
        // 开发者可以在这里绑定自定义事件
        window.core.showToast(window.i18n.t("home.customActions.action3Clicked"), "info");
        if (window.core.isDebugMode()) {
          window.core.logDebug('Custom Action 3 clicked', 'HOME');
        }
      });
    }
  }
}

export { HomePage };
