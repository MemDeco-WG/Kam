/**
 * Kam Usage Page
 * Displays basic usage and common commands for Kam (CLI)
 *
 * Notes:
 * - The page uses window.i18n for translations. If translations are missing,
 *   the keys will fall back to the provided English fallback strings.
 * - Copy to clipboard is supported via the Clipboard API with a fallback for older browsers.
 * - Actions are provided to open the repo and quickly copy the basic build command.
 */
import { marked } from "marked";

class KamPage {
  constructor() {
    this.copyButtons = [];
    this._boundCopyHandler = null;
  }

  render() {
    // Fallback helper for i18n: return key itself if not available
    const t = (key, fallback) => {
      if (window.i18n && window.i18n.t) return window.i18n.t(key);
      return fallback || key;
    };

    // Codes and snippets used in the page
    const snippets = {
      installCargo: "cargo install kam",
      installFromSource:
        "git clone https://github.com/MemDeco-WG/Kam.git\ncd Kam\ncargo build --release",
      initKam: "kam init my_awesome_module --kam",
      initMeta: "kam init my_meta_module --meta",
      initAK3: "kam init my_kernel_module --ak3",
      build: "kam build",
      buildAll: "kam build --all",
      buildBump: "kam build --bump",
      buildRelease: "kam build --release",
      tmplImport: "kam tmpl import templates/meta_template.tar.gz",
      tmplList: "kam tmpl list",
      tmplExport: "kam tmpl export meta_template -o my_template.tar.gz",
      tmplRemove: "kam tmpl remove template_name",
      tmplPath: "kam tmpl path",
      hookExample:
        "hooks/pre-build/0.sync-module-files.sh\n# Add your custom script files here",
      webuiIntegration:
        "webui/ -> src/<module_id>/webroot/ (automatically installed on build)",
    };

    // Build HTML of the page
    return `
      <div class="kam-page page-section">
        <div class="kam-intro status-card">
          <div class="status-card-content">
            <div class="status-info-container">
              <div class="status-title-row">
                <span style="font-weight:600;">${t("kam.title", "Kam 使用指南 / Kam Usage")}</span>
              </div>
              <div class="status-details">
                <div class="status-detail-row">
                  ${t("kam.subtitle", "Kam - KSU/APatch/Magisk Module Builder")}
                </div>
                <div class="status-detail-row" style="margin-top:8px;">
                  <a href="https://github.com/MemDeco-WG/Kam" target="_blank" rel="noopener noreferrer">
                    ${t("kam.openRepo", "Open Kam Repository")}
                  </a>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Installation -->
        <section class="kam-section">
          <h3>${t("kam.install.title", "Installation")}</h3>
          <p>${t("kam.install.desc", "Install via cargo or build from source.")}</p>

          <div class="cmd-block">
            <pre><code id="cmd-install-cargo">${snippets.installCargo}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-install-cargo" aria-label="${t("kam.copy", "Copy")}">
              <span class="material-symbols-rounded">content_copy</span>
              <span class="action-text">${t("kam.copy", "Copy")}</span>
            </button>
          </div>

          <div class="cmd-block">
            <pre><code id="cmd-install-src">${snippets.installFromSource}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-install-src" aria-label="${t("kam.copy", "Copy")}">
              <span class="material-symbols-rounded">content_copy</span>
              <span class="action-text">${t("kam.copy", "Copy")}</span>
            </button>
          </div>
        </section>

        <!-- Init -->
        <section class="kam-section">
          <h3>${t("kam.init.title", "Create a New Module")}</h3>
          <p>${t("kam.init.desc", "Use kam's templates to quickly scaffold a module project.")}</p>

          <div class="cmd-block">
            <pre><code id="cmd-init-kam">${snippets.initKam}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-init-kam">${t("kam.copy", "Copy")}</button>
          </div>

          <div class="cmd-block">
            <pre><code id="cmd-init-meta">${snippets.initMeta}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-init-meta">${t("kam.copy", "Copy")}</button>
          </div>

          <div class="cmd-block">
            <pre><code id="cmd-init-ak3">${snippets.initAK3}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-init-ak3">${t("kam.copy", "Copy")}</button>
          </div>
        </section>

        <!-- Build -->
        <section class="kam-section">
          <h3>${t("kam.build.title", "Build your Module")}</h3>
          <p>${t("kam.build.desc", "Basic build and options for additional use cases and automation")}</p>

          <div class="cmd-block">
            <pre><code id="cmd-build-basic">${snippets.build}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-build-basic">${t("kam.copy", "Copy")}</button>
          </div>

          <div class="cmd-block">
            <pre><code id="cmd-build-all">${snippets.buildAll}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-build-all">${t("kam.copy", "Copy")}</button>
          </div>

          <div class="cmd-block">
            <pre><code id="cmd-build-bump">${snippets.buildBump}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-build-bump">${t("kam.copy", "Copy")}</button>
          </div>

          <div class="cmd-block">
            <pre><code id="cmd-build-release">${snippets.buildRelease}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-build-release">${t("kam.copy", "Copy")}</button>
          </div>

          <div class="cmd-block small-note">
            <small>${t("kam.build.debugNote", "Tip: Run with KAM_DEBUG=1 for debug logs")}</small>
          </div>
        </section>

        <!-- Templates -->
        <section class="kam-section">
          <h3>${t("kam.tmpl.title", "Template Management")}</h3>
          <p>${t("kam.tmpl.desc", "Manage and share templates used for module scaffolding.")}</p>

          <div class="cmd-block">
            <pre><code id="cmd-tmpl-import">${snippets.tmplImport}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-tmpl-import">${t("kam.copy", "Copy")}</button>
          </div>

          <div class="cmd-block">
            <pre><code id="cmd-tmpl-list">${snippets.tmplList}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-tmpl-list">${t("kam.copy", "Copy")}</button>
          </div>

          <div class="cmd-block">
            <pre><code id="cmd-tmpl-export">${snippets.tmplExport}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-tmpl-export">${t("kam.copy", "Copy")}</button>
          </div>

          <div class="cmd-block">
            <pre><code id="cmd-tmpl-remove">${snippets.tmplRemove}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-tmpl-remove">${t("kam.copy", "Copy")}</button>
          </div>

          <div class="cmd-block">
            <pre><code id="cmd-tmpl-path">${snippets.tmplPath}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-tmpl-path">${t("kam.copy", "Copy")}</button>
          </div>
        </section>

        <!-- Hooks and WebUI -->
        <section class="kam-section">
          <h3>${t("kam.hooks.title", "Hook System")}</h3>
          <p>${t("kam.hooks.desc", "Place scripts in hooks/pre-build or hooks/post-build to execute custom automation during builds.")}</p>

          <div class="cmd-block">
            <pre><code id="cmd-hooks">${snippets.hookExample}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-hooks">${t("kam.copy", "Copy")}</button>
          </div>

          <h3>${t("kam.webui.title", "WebUI Integration")}</h3>
          <div class="cmd-block">
            <pre><code id="cmd-webui">${snippets.webuiIntegration}</code></pre>
            <button class="copy-btn small" data-copy="#cmd-webui">${t("kam.copy", "Copy")}</button>
          </div>
        </section>

        <!-- Terminal integration -->
        <section class="kam-section">
          <h3>${t("kam.terminal.title", "Terminal")}</h3>
          <p>${t("kam.terminal.desc", "Run shell commands via KernelSU")}</p>
          <div class="kam-terminal">
            <div class="kam-terminal-controls">
              <input id="kam-terminal-input" class="terminal-input" placeholder="${t("kam.terminal.placeholder", "Enter command...")}" />
              <button id="kam-terminal-run" class="copy-btn small">${t("kam.terminal.run", "Run")}</button>
              <button id="kam-terminal-clear" class="copy-btn small">${t("kam.terminal.clear", "Clear")}</button>
            </div>
            <pre id="kam-terminal-output" class="terminal-output"></pre>
          </div>
        </section>

      </div>
    `;
  }

  getPageActions() {
    const t = (key, fallback) =>
      window.i18n && window.i18n.t ? window.i18n.t(key) : fallback || key;

    return [
      {
        icon: "open_in_new",
        title: t("kam.openRepo", "Open Kam Repository"),
        action: () => {
          try {
            window.open("https://github.com/MemDeco-WG/Kam", "_blank");
          } catch (err) {
            window.core &&
              window.core.showError &&
              window.core.showError(`${err}`, "KamPage");
          }
        },
      },
      {
        icon: "content_copy",
        title: t("kam.build.action.copyBuild", "Copy Build Command"),
        action: () => {
          const cmd = "kam build";
          this._copyText(cmd)
            .then(() => {
              window.core &&
                window.core.showToast &&
                window.core.showToast(
                  t("kam.copySuccess", "Copied to clipboard"),
                  "success",
                );
            })
            .catch(() => {
              window.core &&
                window.core.showToast &&
                window.core.showToast(
                  t("kam.copyFailed", "Copy failed"),
                  "error",
                );
            });
        },
      },
    ];
  }

  async onShow() {
    // Bind copy button event handlers
    // Ensure previous listeners are removed
    this.cleanup();

    // Bound handler
    this._boundCopyHandler = (e) => {
      const btn = e.currentTarget;
      const selector = btn.dataset.copy;
      if (!selector) return;
      const el = document.querySelector(selector);
      if (!el) return;

      const text = el.innerText || el.textContent || "";
      if (!text) return;

      this._copyText(text)
        .then(() => {
          if (window.core && window.core.showToast) {
            window.core.showToast(
              window.i18n
                ? window.i18n.t("kam.copySuccess")
                : "Copied to clipboard",
              "success",
            );
          } else {
            console.log("Copied:", text);
          }
        })
        .catch((err) => {
          if (window.core && window.core.showToast) {
            window.core.showToast(
              window.i18n ? window.i18n.t("kam.copyFailed") : "Copy failed",
              "error",
            );
          } else {
            console.error("Copy failed", err);
          }
        });
    };

    // Find copy buttons and add event listeners
    this.copyButtons = Array.from(
      document.querySelectorAll(".kam-page .copy-btn") || [],
    );
    this.copyButtons.forEach((btn) => {
      btn.addEventListener("click", this._boundCopyHandler);
    });

    // Terminal bindings
    const runBtn = document.getElementById("kam-terminal-run");
    const clearBtn = document.getElementById("kam-terminal-clear");
    const input = document.getElementById("kam-terminal-input");

    if (runBtn) {
      this._boundTerminalHandler = () => {
        const cmd = input && input.value ? input.value.trim() : "";
        if (cmd) this.runTerminalCommand(cmd);
      };
      runBtn.addEventListener("click", this._boundTerminalHandler);
    }

    if (clearBtn) {
      this._boundTerminalClearHandler = () => {
        const out = document.getElementById("kam-terminal-output");
        if (out) out.textContent = "";
      };
      clearBtn.addEventListener("click", this._boundTerminalClearHandler);
    }

    if (input) {
      this._boundTerminalKeyHandler = (e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          const cmd = input.value.trim();
          if (cmd) this.runTerminalCommand(cmd);
        }
      };
      input.addEventListener("keydown", this._boundTerminalKeyHandler);
    }
  }

  cleanup() {
    if (
      this.copyButtons &&
      this.copyButtons.length > 0 &&
      this._boundCopyHandler
    ) {
      this.copyButtons.forEach((btn) => {
        try {
          btn.removeEventListener("click", this._boundCopyHandler);
        } catch (err) {
          // ignore
        }
      });
    }

    // Remove any terminal-related listeners as well if present
    try {
      const runBtn = document.getElementById("kam-terminal-run");
      if (runBtn && this._boundTerminalHandler) {
        runBtn.removeEventListener("click", this._boundTerminalHandler);
      }
    } catch (e) {
      // ignore
    }

    try {
      const input = document.getElementById("kam-terminal-input");
      if (input && this._boundTerminalKeyHandler) {
        input.removeEventListener("keydown", this._boundTerminalKeyHandler);
      }
    } catch (e) {
      // ignore
    }

    this.copyButtons = [];
    this._boundCopyHandler = null;
    this._boundTerminalHandler = null;
    this._boundTerminalKeyHandler = null;
  }

  async loadReadme() {
    const container = document.getElementById("kam-readme");
    if (!container) return;

    // Show a small loading message while we fetch content
    container.innerHTML = `<div class="loading">${window.i18n ? window.i18n.t("kam.readme.loading", "Loading README...") : "Loading README..."}</div>`;

    let md = "";

    // Attempt to fetch local doc first (Kam/docs/user.md)
    try {
      let response = await fetch("/docs/user.md");
      if (response.ok) {
        md = await response.text();
      } else {
        // Fallback to GitHub raw README
        response = await fetch(
          "https://raw.githubusercontent.com/MemDeco-WG/Kam/main/README.md",
        );
        if (response.ok) {
          md = await response.text();
        }
      }
    } catch (err) {
      if (window.core && window.core.isDebugMode()) {
        window.core.logDebug(`Failed to fetch README: ${err.message}`, "KAM");
      }
    }

    if (md) {
      try {
        const html = marked.parse(md);
        container.innerHTML = `<div class="kam-readme-content">${html}</div>`;
      } catch (err) {
        // If marked fails for any reason, put raw MD as text
        container.textContent = md;
      }
    } else {
      container.innerHTML = `<div class="muted">${window.i18n ? window.i18n.t("kam.readme.notFound", "README not found") : "README not found"}</div>`;
    }
  }

  // Helper to copy text to the clipboard with fallback
  _copyText(text) {
    if (!text) return Promise.reject(new Error("No text to copy"));

    // Use Clipboard API when available
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text);
    }

    // Fallback method using a temporary textarea
    return new Promise((resolve, reject) => {
      try {
        const textarea = document.createElement("textarea");
        textarea.value = text;
        textarea.style.position = "fixed";
        textarea.style.top = "-9999px";
        document.body.appendChild(textarea);
        textarea.focus();
        textarea.select();

        const successful = document.execCommand("copy");
        document.body.removeChild(textarea);

        if (successful) resolve();
        else reject(new Error("execCommand returned false"));
      } catch (err) {
        reject(err);
      }
    });
  }
}

export { KamPage };
