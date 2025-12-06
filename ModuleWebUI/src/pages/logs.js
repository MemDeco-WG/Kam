class LogsPage {
  constructor() {
    this.eventHandlers = new Map();
    this.currentLogFile = "";
    this.logFiles = [];
    this.logs = [];
    this.filteredLogs = []; // 筛选后的日志
    this.isLoading = false;
    this.logLevelFilter = 'all'; // 日志级别筛选器
    this.virtualScrollConfig = {
      itemHeight: 30, // 每个日志条目的高度
      containerHeight: 0,
      visibleStart: 0,
      visibleEnd: 0,
      bufferSize: 5, // 缓冲区大小
    };
  }

  render() {
    return `
                <div class="logs-header">
                    <div class="log-file-selector">
                        <label>
                            <span>${window.i18n.t("logs.selectFile")}</span>
                            <select id="log-file-select">
                                <option value="">${window.i18n.t(
                                  "logs.selectPlaceholder"
                                )}</option>
                            </select>
                        </label>
                    </div>
                    <div class="log-level-filter">
                        <label>
                            <span>${window.i18n.t("logs.levelFilter")}</span>
                            <select id="log-level-filter">
                                <option value="all">${window.i18n.t("logs.allLevels")}</option>
                                <option value="debug">${window.i18n.t("logs.debug")}</option>
                                <option value="info">${window.i18n.t("logs.info")}</option>
                                <option value="warn">${window.i18n.t("logs.warn")}</option>
                                <option value="error">${window.i18n.t("logs.error")}</option>
                            </select>
                        </label>
                    </div>
                </div>
                
                <div class="logs-content">
                    <div id="log-display" class="log-display">
                        <div class="logs-status">
                            <span class="material-symbols-rounded">description</span>
                            <p>请选择要查看的日志文件</p>
                        </div>
                    </div>
                </div>
        `;
  }

  async onShow() {
    await this.loadLogFiles();
    this.initEventListeners();
    this.initVirtualScroll();
  }

  initVirtualScroll() {
    const logDisplay = document.getElementById("log-display");
    if (logDisplay) {
      this.virtualScrollConfig.containerHeight = logDisplay.clientHeight;
      logDisplay.addEventListener("scroll", this.handleScroll.bind(this));
    }
  }

  handleScroll(event) {
    const scrollTop = event.target.scrollTop;
    const { itemHeight, bufferSize } = this.virtualScrollConfig;

    this.virtualScrollConfig.visibleStart = Math.max(
      0,
      Math.floor(scrollTop / itemHeight) - bufferSize
    );
    this.virtualScrollConfig.visibleEnd = Math.min(
      this.filteredLogs.length,
      this.virtualScrollConfig.visibleStart +
        Math.ceil(this.virtualScrollConfig.containerHeight / itemHeight) +
        bufferSize * 2
    );

    this.renderVisibleLogs();
  }

  getPageActions() {
    return [
      {
        icon: "refresh",
        title: window.i18n.t("logs.refresh"),
        action: () => this.loadLogs(),
      },
      {
        icon: "clear_all",
        title: window.i18n.t("logs.clear"),
        action: () => this.clearLogs(),
      },
    ];
  }

  initEventListeners() {
    // 日志文件选择
    const logFileSelect = document.getElementById("log-file-select");
    if (logFileSelect) {
      const handler = (e) => this.selectLogFile(e.target.value);
      logFileSelect.addEventListener("change", handler);
      this.eventHandlers.set("log-file-select", {
        element: logFileSelect,
        event: "change",
        handler,
      });
    }
    
    // 日志级别筛选
    const logLevelFilter = document.getElementById("log-level-filter");
    if (logLevelFilter) {
      const handler = (e) => this.filterLogsByLevel(e.target.value);
      logLevelFilter.addEventListener("change", handler);
      this.eventHandlers.set("log-level-filter", {
        element: logLevelFilter,
        event: "change",
        handler,
      });
    }
  }

  cleanup() {
    this.eventHandlers.forEach(({ element, event, handler }) => {
      element.removeEventListener(event, handler);
    });
    this.eventHandlers.clear();

    if (this.refreshInterval) {
      clearInterval(this.refreshInterval);
    }
  }

  async loadLogFiles() {
    try {
      if (!window.core.isKSUEnvironment()) {
        // 浏览器环境模拟
        this.logFiles = ["module.log", "system.log", "error.log", "debug.log"];
        this.populateLogFileSelect();
        return;
      }

      // KSU环境中读取logs目录
      const logsDir = `${window.core.MODULE_PATH}/logs`;
      window.core.execCommand(
        `ls -la "${logsDir}" 2>/dev/null | grep -E '\.(log|txt)$' | awk '{print $NF}' || echo ""`,
        (output, success) => {
          if (success && output.trim()) {
            this.logFiles = output
              .trim()
              .split("\n")
              .filter((file) => file.trim());
          } else {
            this.logFiles = [];
          }
          this.populateLogFileSelect();
        }
      );
    } catch (error) {
      window.core.showError("加载日志文件列表失败", error.message);
      this.logFiles = [];
      this.populateLogFileSelect();
    }
  }

  populateLogFileSelect() {
    const select = document.getElementById("log-file-select");
    if (!select) return;

    // 清空现有选项
    select.innerHTML = `<option value="">${window.i18n.t(
      "logs.selectPlaceholder"
    )}</option>`;

    // 添加日志文件选项
    this.logFiles.forEach((file) => {
      const option = document.createElement("option");
      option.value = file;
      option.textContent = file;
      select.appendChild(option);
    });

    if (this.logFiles.length === 0) {
      const option = document.createElement("option");
      option.value = "";
      option.textContent = window.i18n.t("logs.noFiles");
      option.disabled = true;
      select.appendChild(option);
    }
  }

  selectLogFile(fileName) {
    this.currentLogFile = fileName;
    if (fileName) {
      this.loadLogs();
    } else {
      this.showEmptyState();
    }
  }
  
  /**
   * 根据日志级别筛选日志
   * @param {string} level - 日志级别 (all, debug, info, warn, error)
   */
  filterLogsByLevel(level) {
    this.logLevelFilter = level;
    
    if (level === 'all') {
      this.filteredLogs = [...this.logs];
    } else {
      // 统一转换为小写进行匹配，确保兼容性
      const lowerLevel = level.toLowerCase();
      this.filteredLogs = this.logs.filter(log => {
        const logLevel = (log.level || '').toLowerCase();
        return logLevel === lowerLevel;
      });
    }
    
    // 重置虚拟滚动状态
    this.virtualScrollConfig.visibleStart = 0;
    this.virtualScrollConfig.visibleEnd = 0;
    
    // 重新渲染日志
    this.renderLogs();
    
    if (window.core.isDebugMode()) {
      window.core.logDebug(`Filtered logs by level: ${level}, count: ${this.filteredLogs.length}`, 'LOGS');
    }
  }

  showEmptyState() {
    const logDisplay = document.getElementById("log-display");
    logDisplay.innerHTML = `
            <div class="logs-status">
                <span class="material-symbols-rounded">description</span>
                <p>${window.i18n.t("logs.selectFile")}</p>
            </div>
        `;
  }

  async clearLogs() {
    if (!this.currentLogFile) {
      window.core.showToast(window.i18n.t("logs.selectFirst"), "warning");
      return;
    }

    const confirmed = await window.app.showDialog.confirm(
      window.i18n.t("logs.clear"),
      window.i18n.t("logs.clearConfirm")
    );

    if (confirmed) {
      if (!window.core.isKSUEnvironment()) {
        // 浏览器环境模拟
        this.logs = [];
        this.renderLogs();
        window.core.showToast(window.i18n.t("logs.clearSuccess"), "success");
        return;
      }

      // KSU环境中清空日志文件
      const logFile = this.getLogFilePath();
      window.core.execCommand(`> "${logFile}"`, (output, success) => {
        if (success) {
          this.logs = [];
          this.renderLogs();
          window.core.showToast(window.i18n.t("logs.clearSuccess"), "success");
        } else {
          window.core.showError("清空日志失败", output);
        }
      });
    }
  }

  async loadLogs() {
    if (!this.currentLogFile) {
      this.showEmptyState();
      return;
    }

    this.isLoading = true;
    const logDisplay = document.getElementById("log-display");
    logDisplay.innerHTML = `
            <div class="loading-state">
                <span class="material-symbols-rounded">hourglass_empty</span>
                <span>${window.i18n.t("logs.loading")}</span>
            </div>
        `;

    try {
      if (!window.core.isKSUEnvironment()) {
        // 浏览器环境模拟数据
        this.logs = this.generateMockLogs();
        // 初始化筛选后的日志数组
        this.filterLogsByLevel(this.logLevelFilter);
        this.renderLogs();
        return;
      }

      // KSU环境中读取实际日志
      const logFile = this.getLogFilePath();
      window.core.execCommand(
        `tail -n 100 "${logFile}" 2>/dev/null || echo "日志文件不存在"`,
        (output, success) => {
          try {
            if (success && output && output !== "日志文件不存在") {
              this.logs = output
                .split("\n")
                .filter((line) => line.trim())
                .map((line) => {
                  return this.parseLogLine(line);
                })
                .filter((log) => log !== null);
            } else {
              this.logs = [];
            }
            // 初始化筛选后的日志数组
            this.filterLogsByLevel(this.logLevelFilter);
            this.renderLogs();
          } catch (parseError) {
            window.core.showError("解析日志失败", parseError.message);
            this.renderErrorState();
          } finally {
            this.isLoading = false;
          }
        }
      );
    } catch (error) {
      window.core.showError("加载日志失败", error.message);
      this.renderErrorState();
      this.isLoading = false;
    }
  }

  generateMockLogs() {
    const logTypes = {
      "module.log": [
        {
          level: "INFO",
          message: "模块初始化完成",
          timestamp: new Date(Date.now() - 300000),
        },
        {
          level: "INFO",
          message: "开始监控模块状态",
          timestamp: new Date(Date.now() - 240000),
        },
        {
          level: "WARN",
          message: "检测到配置文件变更",
          timestamp: new Date(Date.now() - 180000),
        },
        {
          level: "INFO",
          message: "状态更新: running",
          timestamp: new Date(Date.now() - 120000),
        },
        {
          level: "INFO",
          message: "模块运行正常",
          timestamp: new Date(Date.now() - 60000),
        },
      ],
      "system.log": [
        {
          level: "INFO",
          message: "系统启动完成",
          timestamp: new Date(Date.now() - 600000),
        },
        {
          level: "INFO",
          message: "WebUI服务已启动",
          timestamp: new Date(Date.now() - 480000),
        },
        {
          level: "WARN",
          message: "内存使用率较高: 78%",
          timestamp: new Date(Date.now() - 360000),
        },
        {
          level: "INFO",
          message: "网络连接正常",
          timestamp: new Date(Date.now() - 240000),
        },
        {
          level: "INFO",
          message: "系统运行稳定",
          timestamp: new Date(Date.now() - 120000),
        },
      ],
      "error.log": [
        {
          level: "ERROR",
          message: "配置文件读取失败: /path/to/config.json",
          timestamp: new Date(Date.now() - 900000),
        },
        {
          level: "ERROR",
          message: "网络连接超时",
          timestamp: new Date(Date.now() - 720000),
        },
        {
          level: "WARN",
          message: "磁盘空间不足警告",
          timestamp: new Date(Date.now() - 540000),
        },
        {
          level: "ERROR",
          message: "模块启动失败，正在重试...",
          timestamp: new Date(Date.now() - 360000),
        },
        {
          level: "INFO",
          message: "错误已修复，模块正常运行",
          timestamp: new Date(Date.now() - 180000),
        },
      ],
      "debug.log": [
        {
          level: "DEBUG",
          message: "调试信息: 变量值检查",
          timestamp: new Date(Date.now() - 400000),
        },
        {
          level: "DEBUG",
          message: "函数调用跟踪",
          timestamp: new Date(Date.now() - 300000),
        },
        {
          level: "DEBUG",
          message: "性能监控数据",
          timestamp: new Date(Date.now() - 200000),
        },
        {
          level: "DEBUG",
          message: "内存使用情况",
          timestamp: new Date(Date.now() - 100000),
        },
      ],
    };

    return logTypes[this.currentLogFile] || [];
  }

  parseLogContent(content) {
    const parsedLogs = content
      .split("\n")
      .filter((line) => line.trim())
      .map((line) => this.parseLogLine(line))
      .filter((log) => log !== null);
    
    // 初始化筛选后的日志数组
    this.logs = parsedLogs;
    this.filterLogsByLevel(this.logLevelFilter);
    
    return parsedLogs;
  }

  parseLogLine(line) {
    if (!line || !line.trim()) return null;

    const trimmedLine = line.trim();
    let match;

    // 格式1: 2024-01-01 12:00:00 [INFO] 消息内容
    match = trimmedLine.match(
      /^(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}(?:\.\d{3})?)\s*\[([A-Z]+)\]\s*(.*)$/i
    );
    if (match) {
      return {
        timestamp: new Date(match[1]),
        level: match[2].toLowerCase(),
        message: match[3].trim(),
      };
    }

    // 格式2: [INFO] 2024-01-01 12:00:00 消息内容
    match = trimmedLine.match(
      /^\[([A-Z]+)\]\s*(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}(?:\.\d{3})?)\s*(.*)$/i
    );
    if (match) {
      return {
        timestamp: new Date(match[2]),
        level: match[1].toLowerCase(),
        message: match[3].trim(),
      };
    }

    // 格式3: 2024-01-01T12:00:00.000Z INFO 消息内容 (ISO格式)
    match = trimmedLine.match(
      /^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z?)\s+([A-Z]+)\s+(.*)$/i
    );
    if (match) {
      return {
        timestamp: new Date(match[1]),
        level: match[2].toLowerCase(),
        message: match[3].trim(),
      };
    }

    // 格式4: INFO 2024-01-01 12:00:00 消息内容
    match = trimmedLine.match(
      /^([A-Z]+)\s+(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}(?:\.\d{3})?)\s+(.*)$/i
    );
    if (match) {
      return {
        timestamp: new Date(match[2]),
        level: match[1].toLowerCase(),
        message: match[3].trim(),
      };
    }

    // 格式5: 时间戳格式 1640995200 INFO message
    match = trimmedLine.match(/^(\d{10,13})\s+([A-Z]+)\s+(.*)$/i);
    if (match) {
      const timestamp = match[1].length === 10 ? parseInt(match[1]) * 1000 : parseInt(match[1]);
      return {
        timestamp: new Date(timestamp),
        level: match[2].toLowerCase(),
        message: match[3].trim(),
      };
    }

    // 格式6: 2024/01/01 12:00:00 INFO 消息内容 (斜杠分隔日期)
    match = trimmedLine.match(
      /^(\d{4}\/\d{2}\/\d{2}\s+\d{2}:\d{2}:\d{2}(?:\.\d{3})?)\s+([A-Z]+)\s+(.*)$/i
    );
    if (match) {
      return {
        timestamp: new Date(match[1].replace(/\//g, '-')),
        level: match[2].toLowerCase(),
        message: match[3].trim(),
      };
    }

    // 格式7: Jan 01 12:00:00 INFO 消息内容 (syslog格式)
    match = trimmedLine.match(
      /^([A-Z][a-z]{2}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2})\s+([A-Z]+)\s+(.*)$/i
    );
    if (match) {
      const currentYear = new Date().getFullYear();
      const dateStr = `${currentYear} ${match[1]}`;
      return {
        timestamp: new Date(dateStr),
        level: match[2].toLowerCase(),
        message: match[3].trim(),
      };
    }

    // 格式8: 12:00:00.123 [INFO] 消息内容 (只有时间)
    match = trimmedLine.match(
      /^(\d{2}:\d{2}:\d{2}(?:\.\d{3})?)\s*\[([A-Z]+)\]\s*(.*)$/i
    );
    if (match) {
      const today = new Date();
      const timeStr = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}-${String(today.getDate()).padStart(2, '0')} ${match[1]}`;
      return {
        timestamp: new Date(timeStr),
        level: match[2].toLowerCase(),
        message: match[3].trim(),
      };
    }

    // 格式9: 检测行中是否包含日志级别关键词
    const levelKeywords = ['ERROR', 'WARN', 'WARNING', 'INFO', 'DEBUG', 'TRACE', 'FATAL'];
    for (const keyword of levelKeywords) {
      const regex = new RegExp(`\\b${keyword}\\b`, 'i');
      if (regex.test(trimmedLine)) {
        // 尝试提取时间戳
        const timeMatch = trimmedLine.match(/(\d{4}[-\/]\d{2}[-\/]\d{2}[\sT]\d{2}:\d{2}:\d{2}(?:\.\d{3})?(?:Z|[+-]\d{2}:\d{2})?)|\b(\d{10,13})\b/);
        let timestamp = new Date();
        if (timeMatch) {
          if (timeMatch[1]) {
            timestamp = new Date(timeMatch[1].replace(/\//g, '-'));
          } else if (timeMatch[2]) {
            const ts = timeMatch[2].length === 10 ? parseInt(timeMatch[2]) * 1000 : parseInt(timeMatch[2]);
            timestamp = new Date(ts);
          }
        }
        
        return {
          timestamp: timestamp,
          level: keyword.toLowerCase() === 'warning' ? 'warn' : keyword.toLowerCase(),
          message: trimmedLine,
        };
      }
    }

    // 格式10: 默认格式，将整行作为消息内容
    return {
      timestamp: new Date(),
      level: "info",
      message: trimmedLine,
    };
  }

  formatRelativeTime(timestamp) {
    const now = new Date();
    const logTime = new Date(timestamp);
    const diffMs = now - logTime;
    const diffMinutes = Math.floor(diffMs / (1000 * 60));
    const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    // 如果是无效时间，返回原始值
    if (isNaN(logTime.getTime())) {
      return timestamp.toString();
    }

    // 1分钟内显示"现在"
    if (diffMinutes < 1) {
      return "现在";
    }

    // 1小时内显示"X分钟前"
    if (diffMinutes < 60) {
      return `${diffMinutes}分钟前`;
    }

    // 今天显示"今天 HH:MM"
    if (diffDays === 0) {
      return `今天 ${logTime.getHours().toString().padStart(2, "0")}:${logTime
        .getMinutes()
        .toString()
        .padStart(2, "0")}`;
    }

    // 昨天显示"昨天 HH:MM"
    if (diffDays === 1) {
      return `昨天 ${logTime.getHours().toString().padStart(2, "0")}:${logTime
        .getMinutes()
        .toString()
        .padStart(2, "0")}`;
    }

    // 一周内显示"X天前"
    if (diffDays < 7) {
      return `${diffDays}天前`;
    }

    // 超过一周显示完整日期
    return `${logTime.getMonth() + 1}/${logTime.getDate()} ${logTime
      .getHours()
      .toString()
      .padStart(2, "0")}:${logTime.getMinutes().toString().padStart(2, "0")}`;
  }

  renderLogs() {
    const logDisplay = document.getElementById("log-display");
    if (!logDisplay) return;

    if (this.filteredLogs.length === 0) {
      const message = this.logs.length === 0 ? 
        window.i18n.t("logs.noData") : 
        window.i18n.t("logs.noFilteredData");
      logDisplay.innerHTML = `
                <div class="logs-status">
                    <span class="material-symbols-rounded">inbox</span>
                    <p>${message}</p>
                </div>
            `;
      return;
    }

    // 初始化虚拟滚动
    this.virtualScrollConfig.containerHeight = logDisplay.clientHeight;
    this.virtualScrollConfig.visibleEnd = Math.min(
      this.filteredLogs.length,
      Math.ceil(
        this.virtualScrollConfig.containerHeight /
          this.virtualScrollConfig.itemHeight
      ) +
        this.virtualScrollConfig.bufferSize * 2
    );

    this.renderVisibleLogs();

    // 滚动到底部显示最新日志
    setTimeout(() => {
      logDisplay.scrollTop = logDisplay.scrollHeight;
    }, 100);
  }

  renderVisibleLogs() {
    const logDisplay = document.getElementById("log-display");
    if (!logDisplay || this.filteredLogs.length === 0) return;

    const { visibleStart, visibleEnd, itemHeight } = this.virtualScrollConfig;
    const totalHeight = this.filteredLogs.length * itemHeight;
    const offsetY = visibleStart * itemHeight;

    const visibleLogs = this.filteredLogs.slice(visibleStart, visibleEnd);
    const logEntries = visibleLogs
      .map((log, index) => {
        const relativeTime = this.formatRelativeTime(log.timestamp);
        const fullTime = log.timestamp.toLocaleString("zh-CN", {
          year: "numeric",
          month: "2-digit",
          day: "2-digit",
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
        });

        const levelClass = log.level.toLowerCase();
        const levelBadge = `<span class="log-level-badge ${levelClass}">${log.level.toUpperCase()}</span>`;

        return `
                <div class="log-entry ${levelClass}" style="position: absolute; top: ${
          (visibleStart + index) * itemHeight
        }px; width: 100%; box-sizing: border-box;">
                    <span class="log-timestamp" title="${fullTime}">${relativeTime}</span>
                    ${levelBadge}
                    <span class="log-message">${this.escapeHtml(
                      log.message
                    )}</span>
                </div>
            `;
      })
      .join("");

    logDisplay.innerHTML = `
            <div class="log-entries" style="position: relative; height: ${totalHeight}px;">
                ${logEntries}
            </div>
        `;
  }

  renderErrorState() {
    const logDisplay = document.getElementById("log-display");
    logDisplay.innerHTML = `
            <div class="logs-status">
                <span class="material-symbols-rounded">error</span>
                <p>${window.i18n.t("logs.loadFailed")}</p>
                <button onclick="window.app.pages.logs.loadLogs()">${window.i18n.t(
                  "common.retry"
                )}</button>
            </div>
        `;
  }

  escapeHtml(text) {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }

  getLogFilePath() {
    if (!this.currentLogFile) {
      return `${window.core.MODULE_PATH}/logs/module.log`;
    }
    return `${window.core.MODULE_PATH}/logs/${this.currentLogFile}`;
  }
}

export { LogsPage };
