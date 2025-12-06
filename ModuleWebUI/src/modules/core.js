/**
 * AMMF WebUI 核心功能模块
 * 提供Shell命令执行能力
 * 支持KernelSU和MMRL WebUI X
 */

export const Core = {
  // 模块路径
  MODULE_PATH: "/data/adb/modules//",

  // Debug日志文件路径
  DEBUG_LOG_PATH: "/data/adb//debug.log",

  // 检测是否在KSU环境中
  isKSUEnvironment() {
    return typeof ksu !== "undefined" && ksu.toast;
  },
  // 检查是否启用debug模式
  isDebugMode() {
    const saved = localStorage.getItem("modulewebui-settings");
    if (saved) {
      const settings = JSON.parse(saved);
      return settings.debugMode || false;
    }
    return false;
  },

  // 记录debug信息到文件 (English only for debug messages)
  logDebug(message, type = "INFO") {
    if (!this.isDebugMode()) return;

    const timestamp = new Date().toISOString();
    const logEntry = `[${timestamp}] [${type}] ${message}\n`;

    // 直接写入debug日志文件，避免递归调用
    this._execCommandInternal(`echo "${logEntry}" >> ${this.DEBUG_LOG_PATH}`);
  },
  // 检查命令输出是否包含错误
  _isCommandError(output) {
    if (!output || typeof output !== "string") return false;

    const errorPatterns = [
      /error:/i,
      /failed/i,
      /not found/i,
      /permission denied/i,
      /no such file/i,
      /command not found/i,
      /ksu\.exec is not defined/i,
    ];

    return errorPatterns.some((pattern) => pattern.test(output));
  },
  _execCommandInternal(command, callback) {
    const callbackName = `callback_${Date.now()}_${Math.floor(
      Math.random() * 1000
    )}`;

    window[callbackName] = (errno, stdout, stderr) => {
      if (callback) {
        // 处理ksu.exec返回的三个参数：errno, stdout, stderr
        const output = stdout || stderr || "";
        const isError = errno !== 0 || (stderr && stderr.trim() !== "");
        callback(output, !isError, { errno, stdout, stderr });
      }
      delete window[callbackName];
    };

    if (typeof ksu !== "undefined" && ksu.exec) {
      // 传递选项参数给ksu.exec
      ksu.exec(command, JSON.stringify({}), callbackName);
    } else {
      const errorMsg = "Error: ksu.exec is not defined.";
      console.error(errorMsg);
      if (callback) {
        callback(errorMsg, false, { errno: -1, stdout: "", stderr: errorMsg });
      }
    }
  },

  /**
   * 执行单个 shell 命令。
   * @param {string} command 要执行的 shell 命令。
   * @param {function(string, boolean, object): void} [callback] 命令执行后的回调函数，接收命令输出、是否成功和详细结果作为参数。
   */
  execCommand(command, callback) {
    // Debug模式下记录命令执行 (English only for debug)
    if (this.isDebugMode()) {
      this.logDebug(`Executing command: ${command}`, "EXEC");
      this.showToast(`[DEBUG] Executing command: ${command}`, "info");
    }

    this._execCommandInternal(command, (output, isSuccess, details) => {
      // 使用新的错误检测逻辑，优先使用ksu返回的errno
      const isError = !isSuccess || this._isCommandError(output);

      // Debug模式下记录命令输出 (English only for debug)
      if (this.isDebugMode()) {
        const status = isError ? "ERROR" : "SUCCESS";
        this.logDebug(`Command output [${status}]: ${output}`, "OUTPUT");
        if (details) {
          this.logDebug(
            `Command details - errno: ${details.errno}, stdout length: ${
              details.stdout ? details.stdout.length : 0
            }, stderr length: ${details.stderr ? details.stderr.length : 0}`,
            "OUTPUT"
          );
        }
        this.showToast(
          `[DEBUG] Command ${status}: ${output.substring(0, 100)}${
            output.length > 100 ? "..." : ""
          }`,
          isError ? "error" : "info"
        );
      }

      if (callback) {
        callback(output, !isError, details);
      }
    });
  },

  /**
   * 执行命令并返回Promise（与原始exec函数兼容）
   * @param {string} command 要执行的命令
   * @param {object} options 选项参数
   * @returns {Promise<{errno: number, stdout: string, stderr: string}>}
   */
  exec(command, options = {}) {
    return new Promise((resolve, reject) => {
      // 生成唯一的回调函数名
      const callbackFuncName = `callback_${Date.now()}_${Math.floor(
        Math.random() * 1000
      )}`;

      // 定义成功回调函数
      window[callbackFuncName] = (errno, stdout, stderr) => {
        resolve({ errno, stdout, stderr });
        cleanup(callbackFuncName);
      };

      function cleanup(successName) {
        delete window[successName];
      }

      try {
        if (typeof ksu !== "undefined" && ksu.exec) {
          ksu.exec(command, JSON.stringify(options), callbackFuncName);
        } else {
          reject(new Error("ksu.exec is not defined"));
          cleanup(callbackFuncName);
        }
      } catch (error) {
        reject(error);
        cleanup(callbackFuncName);
      }
    });
  },

  /**
   * 显示错误消息
   * @param {string} error 错误信息
   * @param {string} context 错误上下文
   */
  showError(error, context = "") {
    let errorMessage = error;

    // 检查常见错误并提供本地化消息
    if (error.includes("ksu.exec is not defined")) {
      errorMessage = window.i18n ? window.i18n.t("errors.ksuNotDefined") : "KSU not defined";
    } else if (error.includes("not found")) {
      errorMessage = window.i18n ? window.i18n.t("errors.fileNotFound") : "File not found";
    } else if (error.includes("permission denied")) {
      errorMessage = window.i18n ? window.i18n.t("errors.permissionDenied") : "Permission denied";
    } else if (error.includes("failed")) {
      errorMessage = window.i18n ? window.i18n.t("errors.execFailed") : "Execution failed";
    }

    if (context) {
      errorMessage = `${context}: ${errorMessage}`;
    }

    this.showToast(errorMessage, "error");

    // Debug模式下记录详细错误信息 (English only for debug)
    if (this.isDebugMode()) {
      this.logDebug(`Error occurred [${context}]: ${error}`, "ERROR");
    }
  },

  /**
   * 在 UI 中显示一个 Toast 消息。
   * @param {string} message 要显示的消息内容。
   * @param {'success' | 'error' | 'info' | 'warning'} [type='info'] 消息类型，影响显示样式。
   */
  showToast(message, type = "info") {
    // 记录所有toast输出到debug日志（仅非debug toast避免递归）(English only for debug)
    if (this.isDebugMode() && !message.includes("[DEBUG]")) {
      this.logDebug(
        `Toast display: [${type.toUpperCase()}] ${message}`,
        "TOAST"
      );
    }

    // 获取设置管理器实例
    const settingsManager = window.settingsManager;
    const useNativeToast = settingsManager
      ? settingsManager.getSetting("useNativeToast")
      : this.isKSUEnvironment();

    // 如果启用原生toast且在KSU环境中，使用原生toast
    if (useNativeToast && this.isKSUEnvironment()) {
      ksu.toast(message);
      return;
    }

    // 否则使用自定义toast
    const toastContainer = document.getElementById("toast-container");
    if (!toastContainer) {
      console.warn("Toast container not found.");
      // 如果找不到容器，尝试使用原生toast作为备用
      if (this.isKSUEnvironment()) {
        ksu.toast(message);
      } else {
        console.log(message); // 最后的备用方案
      }
      return;
    }

    const toast = document.createElement("div");
    toast.className = `toast toast-${type}`;
    toast.textContent = message;

    toastContainer.appendChild(toast);

    // 动画效果
    setTimeout(() => {
      toast.classList.add("show");
    }, 10);

    setTimeout(() => {
      toast.classList.remove("show");
      toast.classList.add("hide");
      toast.addEventListener(
        "transitionend",
        () => {
          toast.remove();
        },
        { once: true }
      );
    }, 3000);
  },
};
export default Core;
window.core = Core;
