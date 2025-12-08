class DialogManager {
  constructor() {
    this.confirmDialog = document.getElementById("confirm-dialog");
    this.confirmTitle = document.getElementById("confirm-title");
    this.confirmContent = document.getElementById("confirm-content");
    this.confirmCancel = document.getElementById("confirm-cancel");
    this.confirmOk = document.getElementById("confirm-ok");

    // 存储对话框的点击处理器，用于动态控制
    this.dialogClickHandlers = new Map();

    this.setupEventListeners();
    this.createDynamicDialogs();

    // 为确认对话框添加点击空白处关闭功能
    this.addClickOutsideHandler(this.confirmDialog);
  }

  createDynamicDialogs() {
    // 创建通用对话框容器
    this.createGenericDialog();
    this.createInputDialog();
    this.createListDialog();
    this.createPopupDialog();
  }

  // 统一的点击外部关闭处理器
  addClickOutsideHandler(dialog, closable = true) {
    const handler = (e) => {
      if (e.target === dialog) {
        const handlerInfo = this.dialogClickHandlers.get(dialog);
        if (handlerInfo && handlerInfo.closable) {
          this.closeDialogWithAnimation(dialog);
        }
      }
    };

    dialog.addEventListener("click", handler);
    this.dialogClickHandlers.set(dialog, { handler, closable });
  }

  // 动态控制对话框是否可点击外部关闭
  setDialogClosable(dialog, closable) {
    const handlerInfo = this.dialogClickHandlers.get(dialog);
    if (handlerInfo) {
      handlerInfo.closable = closable;
    }
  }

  // 统一的对话框创建方法
  createDialog(id, className, innerHTML) {
    const dialog = document.createElement("dialog");
    dialog.id = id;
    dialog.className = className;
    dialog.innerHTML = innerHTML;
    document.body.appendChild(dialog);
    this.addClickOutsideHandler(dialog);
    return dialog;
  }

  // 统一显示对话框的方法
  show(dialog, options = {}) {
    const { closable = true } = options;
    this.setDialogClosable(dialog, closable);
    this.showDialogWithAnimation(dialog);
    return new Promise((resolve) => {
      dialog._resolve = resolve;
    });
  }

  // 统一关闭对话框的方法
  close(dialog, result = null) {
    this.closeDialogWithAnimation(dialog);
    if (dialog._resolve) {
      dialog._resolve(result);
      delete dialog._resolve;
    }
  }

  createGenericDialog() {
    this.genericDialog = this.createDialog(
      "generic-dialog",
      "generic-dialog",
      `
            <div class="dialog-header">
                <h2 id="generic-title">标题</h2>
                <button id="generic-close" class="dialog-close-btn" style="display: flex;">
                    <span class="material-symbols-rounded">close</span>
                </button>
            </div>
            <div class="dialog-content" id="generic-content">
                <!-- 动态内容 -->
            </div>
            <fieldset id="generic-actions">
                <!-- 动态按钮 -->
            </fieldset>
        `
    );
  }

  createInputDialog() {
    this.inputDialog = this.createDialog(
      "input-dialog",
      "dialog",
      `
            <div class="dialog-header">
                <h2 id="input-dialog-title"></h2>
            </div>
            <div class="dialog-content simple">
                <p id="input-dialog-message"></p>
                <input type="text" id="input-dialog-input" class="dialog-input" />
            </div>
            <div class="dialog-actions">
                <button id="input-dialog-cancel" class="text">取消</button>
                <button id="input-dialog-confirm" class="filled">确定</button>
            </div>
        `
    );
  }

  createListDialog() {
    this.listDialog = this.createDialog(
      "list-dialog",
      "dialog",
      `
            <div class="dialog-header">
                <h2 id="list-dialog-title"></h2>
            </div>
            <div class="dialog-content complex">
                <div id="list-dialog-list" class="dialog-list"></div>
            </div>
            <div class="dialog-actions">
                <button id="list-dialog-cancel" class="text">取消</button>
            </div>
        `
    );
  }

  createPopupDialog() {
    this.popupDialog = this.createDialog(
      "popup-dialog",
      "popup-dialog",
      `
            <div class="popup-content" id="popup-content">
                <!-- 动态内容 -->
            </div>
        `
    );
  }

  setupEventListeners() {
    if (
      typeof window.window.core !== "undefined" &&
      window.window.core.isDebugMode &&
      window.window.core.isDebugMode()
    ) {
      window.window.core.logDebug(
        "DialogManager event listeners setup completed",
        "DIALOG"
      );
    }

    // 确认对话框事件
    this.confirmCancel.addEventListener("click", () => {
      if (
        typeof window.window.core !== "undefined" &&
        window.window.core.isDebugMode &&
        window.window.core.isDebugMode()
      ) {
        window.window.core.logDebug("User clicked cancel button", "DIALOG");
      }
      this.closeDialogWithAnimation(this.confirmDialog);
    });
  }

  showDialogWithAnimation(dialog) {
    if (
      typeof window.window.core !== "undefined" &&
      window.window.core.isDebugMode &&
      window.window.core.isDebugMode()
    ) {
      window.window.core.logDebug(
        `Start showing dialog animation: ${dialog.id}`,
        "DIALOG"
      );
    }

    dialog.showModal();
    // 触发进入动画
    setTimeout(() => {
      dialog.classList.add("showing");
      if (
        typeof window.window.core !== "undefined" &&
        window.window.core.isDebugMode &&
        window.window.core.isDebugMode()
      ) {
        window.window.core.logDebug(
          `Dialog show animation completed: ${dialog.id}`,
          "DIALOG"
        );
      }
    }, 10);
  }

  closeDialogWithAnimation(dialog) {
    if (
      typeof window.window.core !== "undefined" &&
      window.window.core.isDebugMode &&
      window.window.core.isDebugMode()
    ) {
      window.window.core.logDebug(
        `Start closing dialog animation: ${dialog.id}`,
        "DIALOG"
      );
    }

    dialog.classList.remove("showing");
    dialog.classList.add("closing");

    // 等待动画完成后关闭对话框
    setTimeout(() => {
      dialog.close();
      dialog.classList.remove("closing");
      if (
        typeof window.window.core !== "undefined" &&
        window.window.core.isDebugMode &&
        window.window.core.isDebugMode()
      ) {
        window.window.core.logDebug(
          `Dialog close animation completed: ${dialog.id}`,
          "DIALOG"
        );
      }
    }, 200); // 与CSS动画时间一致
  }

  /**
   * 显示通用对话框
   * @param {Object} options - 对话框选项
   * @param {string} options.title - 标题
   * @param {string} options.content - 内容（HTML）
   * @param {Array} options.buttons - 按钮配置 [{text, style, action}]
   * @param {boolean} options.closable - 是否可关闭
   * @param {boolean} options.isComplex - 内容是否复杂
   * @returns {Promise} - 用户操作结果
   */
  showGeneric(options) {
    return new Promise((resolve) => {
      const {
        title,
        content,
        buttons = [],
        closable = true,
        isComplex = false,
      } = options;

      // 设置标题和内容
      const titleEl = document.getElementById("generic-title");
      if (titleEl) {
        titleEl.textContent = title;
      }
      
      const contentEl = document.getElementById("generic-content");
      if (contentEl) {
        contentEl.innerHTML = content;
        // 根据内容复杂度设置样式类
        contentEl.className = `dialog-content ${
          isComplex ? "complex" : "simple"
        }`;
      }

      // 设置关闭按钮
      const closeBtn = document.getElementById("generic-close");
      if (closeBtn) {
        if (closable) {
          closeBtn.style.display = "flex";
          closeBtn.onclick = () => {
            this.closeDialogWithAnimation(this.genericDialog);
            resolve(null);
          };
        } else {
          closeBtn.style.display = "none";
        }
      } else if (window.core && window.core.isDebugMode()) {
        window.core.logDebug('Generic close button not found', 'DIALOG');
      }

      // 生成按钮
      const actionsContainer = document.getElementById("generic-actions");
      if (actionsContainer) {
        actionsContainer.innerHTML = "";

        buttons.forEach((btn, index) => {
          const button = document.createElement("button");
          button.textContent = btn.text;
          if (btn.style) button.className = btn.style;
          button.onclick = () => {
            this.closeDialogWithAnimation(this.genericDialog);
            if (btn.action) btn.action();
            resolve(index);
          };
          actionsContainer.appendChild(button);
        });
      } else if (window.core && window.core.isDebugMode()) {
        window.core.logDebug('Generic actions container not found', 'DIALOG');
      }

      this.showDialogWithAnimation(this.genericDialog);
    });
  }

  /**
   * 显示输入对话框
   * @param {string} title - 标题
   * @param {string} message - 提示信息
   * @param {string} placeholder - 输入框占位符
   * @param {string} defaultValue - 默认值
   * @returns {Promise<string|null>} - 用户输入结果
   */
  showInput(title, message, placeholder = "", defaultValue = "") {
    return new Promise((resolve) => {
      document.getElementById("input-title").textContent = title;
      document.getElementById("input-message").textContent = message;

      const inputField = document.getElementById("input-field");
      inputField.placeholder = placeholder;
      inputField.value = defaultValue;

      // 移除旧的事件监听器
      const cancelBtn = document.getElementById("input-cancel");
      const okBtn = document.getElementById("input-ok");

      const newCancelBtn = cancelBtn.cloneNode(true);
      const newOkBtn = okBtn.cloneNode(true);

      cancelBtn.parentNode.replaceChild(newCancelBtn, cancelBtn);
      okBtn.parentNode.replaceChild(newOkBtn, okBtn);

      // 添加事件监听器
      newCancelBtn.onclick = () => {
        this.closeDialogWithAnimation(this.inputDialog);
        resolve(null);
      };

      newOkBtn.onclick = () => {
        const value = inputField.value.trim();
        this.closeDialogWithAnimation(this.inputDialog);
        resolve(value);
      };

      // 回车键确认
      inputField.onkeydown = (e) => {
        if (e.key === "Enter") {
          newOkBtn.click();
        }
      };

      this.showDialogWithAnimation(this.inputDialog);
      setTimeout(() => inputField.focus(), 100);
    });
  }

  /**
   * 显示列表选择对话框
   * @param {string} title - 标题
   * @param {Array} items - 列表项 [{text, value, icon}]
   * @returns {Promise<any|null>} - 用户选择结果
   */
  showList(title, items) {
    return new Promise((resolve) => {
      document.getElementById("list-title").textContent = title;

      const container = document.getElementById("list-container");
      container.innerHTML = "";

      items.forEach((item, index) => {
        const listItem = document.createElement("div");
        listItem.className = "dialog-list-item";
        listItem.innerHTML = `
                    ${
                      item.icon
                        ? `<span class="material-symbols-rounded">${item.icon}</span>`
                        : ""
                    }
                    <span>${item.text}</span>
                `;
        listItem.onclick = () => {
          this.closeDialogWithAnimation(this.listDialog);
          resolve(item.value !== undefined ? item.value : item.text);
        };
        container.appendChild(listItem);
      });

      // 取消按钮
      const cancelBtn = document.getElementById("list-cancel");
      const newCancelBtn = cancelBtn.cloneNode(true);
      cancelBtn.parentNode.replaceChild(newCancelBtn, cancelBtn);

      newCancelBtn.onclick = () => {
        this.closeDialogWithAnimation(this.listDialog);
        resolve(null);
      };

      this.showDialogWithAnimation(this.listDialog);
    });
  }

  /**
   * 显示确认对话框
   * @param {string} title - 对话框标题
   * @param {string} content - 对话框内容
   * @returns {Promise<boolean>} - 用户选择结果
   */
  showConfirm(title, content) {
    // 检查window.core是否可用以及是否为debug模式
    if (
      typeof window.window.core !== "undefined" &&
      window.window.core.isDebugMode &&
      window.window.core.isDebugMode()
    ) {
      window.window.core.logDebug(
        "DIALOG",
        `Show confirmation dialog: ${title}`
      );
      window.window.core.showToast("[DEBUG] Showing confirm dialog", "info");
    }

    return new Promise((resolve) => {
      this.confirmTitle.textContent = title;
      this.confirmContent.textContent = content;

      // 移除之前的事件监听器
      const newConfirmOk = this.confirmOk.cloneNode(true);
      this.confirmOk.parentNode.replaceChild(newConfirmOk, this.confirmOk);
      this.confirmOk = newConfirmOk;

      // 添加新的事件监听器
      this.confirmOk.addEventListener("click", () => {
        if (
          typeof window.window.core !== "undefined" &&
          window.window.core.isDebugMode &&
          window.window.core.isDebugMode()
        ) {
          window.window.core.logDebug("DIALOG", "User clicked confirm");
        }
        this.closeDialogWithAnimation(this.confirmDialog);
        resolve(true);
      });

      // 处理对话框关闭事件
      const handleClose = () => {
        if (
          typeof window.window.core !== "undefined" &&
          window.window.core.isDebugMode &&
          window.window.core.isDebugMode()
        ) {
          window.window.core.logDebug(
            "DIALOG",
            "Dialog closed, user cancelled"
          );
        }
        this.confirmDialog.removeEventListener("close", handleClose);
        resolve(false);
      };

      this.confirmDialog.addEventListener("close", handleClose);
      this.showDialogWithAnimation(this.confirmDialog);
    });
  }

  /**
   * 显示弹出式对话框
   * @param {Object} options - 配置选项
   * @param {string} options.content - HTML内容
   * @param {boolean} options.closable - 是否可关闭
   * @returns {Promise} - Promise对象
   */
  showPopup(options) {
    const { content, closable = true } = options;

    // 设置内容
    const contentEl = document.getElementById("popup-content");
    contentEl.innerHTML = content;

    // 使用统一的显示方法
    return this.show(this.popupDialog, { closable });
  }

  /**
   * 简化的API - 快速显示不同类型的对话框
   */
  dialog = {
    // 显示确认对话框
    confirm: (title, content) => this.showConfirm(title, content),

    // 显示输入对话框
    input: (title, message, placeholder = "", defaultValue = "") =>
      this.showInput(title, message, placeholder, defaultValue),

    // 显示列表选择对话框
    list: (title, items) => this.showList(title, items),

    // 显示通用对话框
    generic: (options) => this.showGeneric(options),

    // 显示弹出对话框
    popup: (options) => this.showPopup(options),

    // 关闭指定对话框
    close: (dialog, result) => this.close(dialog, result),
  };
}

// 创建全局实例
window.DialogManager = new DialogManager();
