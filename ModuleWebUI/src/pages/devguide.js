import { marked } from "marked";

/**
 * DevGuidePage
 *
 * Simple page to render the development guide from `docs/develop.md`
 * Supports language fallbacks: tries localized file first (developer docs),
 * then falls back to the English version (develop.en.md / develop.md).
 */
class DevGuidePage {
  constructor() {
    this.copyButtons = [];
    this._boundLinkHandler = null;
  }

  // i18n helper
  t(key, fallback) {
    if (window.i18n && window.i18n.t) {
      return window.i18n.t(key, fallback);
    }
    return fallback || key;
  }

  render() {
    return `
      <div class="devguide-page page-section">
        <div class="status-card devguide-intro">
          <div class="status-card-content">
            <div class="status-info-container">
              <div class="status-title-row">
                <span style="font-weight:600;">${this.t("devguide.title", "Developer Guide")}</span>
              </div>
              <div class="status-details">
                <div class="status-detail-row">${this.t(
                  "devguide.subtitle",
                  "Developer guide and module/plugin documentation"
                )}</div>
              </div>
            </div>
          </div>
        </div>

        <section class="devguide-section">
          <div class="doc-block">
            <div id="devguide-content" class="devguide-content loading">
              <div class="loading-spinner"></div>
              <div class="loading-text">${this.t("devguide.loading", "Loading developer guide...")}</div>
            </div>
          </div>
        </section>
      </div>
    `;
  }

  getPageActions() {
    return [
      {
        icon: "open_in_new",
        title: this.t("devguide.openRepo", "Open repository"),
        action: () => {
          try {
            window.open("https://github.com/MemDeco-WG/ModuleWebUI", "_blank");
          } catch (err) {
            window.core && window.core.showError && window.core.showError(`${err}`, "DevGuidePage");
          }
        },
      },
      {
        icon: "content_copy",
        title: this.t("devguide.copyDocLink", "Copy doc link"),
        action: () => {
          const raw = this._getDocRawUrl();
          this._copyText(raw)
            .then(() => {
              if (window.core && window.core.showToast) {
                window.core.showToast(this.t("devguide.copySuccess", "Copied to clipboard"), "success");
              } else {
                console.log("Copied:", raw);
              }
            })
            .catch(() => {
              if (window.core && window.core.showToast) {
                window.core.showToast(this.t("devguide.copyFailed", "Copy failed"), "error");
              }
            });
        },
      },
      {
        icon: "refresh",
        title: this.t("devguide.refresh", "Refresh"),
        action: () => {
          this._loadDoc();
        },
      },
    ];
  }

  async onShow() {
    // Render the doc content
    await this._loadDoc();
    this._bindLinks();
  }

  cleanup() {
    // Remove bound link handler
    if (this._boundLinkHandler) {
      document.removeEventListener("click", this._boundLinkHandler);
      this._boundLinkHandler = null;
    }
  }

  /**
   * Private helper:
   * Determine preferred doc paths for the developer guide.
   * We prefer localized path like: /docs/develop.md OR /docs/develop.<lang>.md
   */
  _getDocPaths() {
    const paths = [];
    const defaultPath = "/docs/develop.md";
    const enPath = "/docs/develop.en.md";
    const zhPath = "/docs/develop.md"; // docs/develop.md is primary in repo (Chinese by default)

    // If i18n has a language, try that first
    let lang = "en";
    if (window.i18n && window.i18n.getLanguage) {
      lang = window.i18n.getLanguage() || "en";
    } else if (window.i18n && window.i18n.lang) {
      lang = window.i18n.lang || "en";
    }

    if (/^zh/i.test(lang)) {
      paths.push(zhPath, defaultPath, enPath);
    } else {
      // try English first, then default, then localized
      paths.push(enPath, defaultPath, zhPath);
    }

    // Remove duplicates
    const unique = Array.from(new Set(paths));
    return unique;
  }

  _getDocRawUrl() {
    // Return the canonical doc raw path (prefer English)
    return "/docs/develop.md";
  }

  async _loadDoc() {
    const container = document.getElementById("devguide-content");
    if (!container) return;

    container.innerHTML = `<div class="loading-spinner"></div><div class="loading-text">${this.t("devguide.loading", "Loading developer guide...")}</div>`;
    container.className = "devguide-content loading";

    const paths = this._getDocPaths();
    let content = null;
    for (const p of paths) {
      try {
        const resp = await fetch(p, { cache: "no-cache" });
        if (!resp.ok) {
          continue;
        }
        content = await resp.text();
        if (content && content.trim().length > 0) {
          break;
        }
      } catch (err) {
        // Try next
      }
    }

    if (!content) {
      container.innerHTML = `
        <div class="status-card">
          <div class="status-card-content">
            <div class="status-info-container">
              <div class="status-title-row">
                <span style="font-weight:600;">${this.t("devguide.notFound", "Developer guide not found")}</span>
              </div>
              <div class="status-details">
                <div class="status-detail-row">${this.t("devguide.tryAgain", "Please ensure the `docs/develop.md` file exists.")}</div>
                <div class="status-detail-row">
                  <a href="/docs/develop.md" target="_blank">${this.t("devguide.openRaw", "Open Raw")}</a>
                </div>
              </div>
            </div>
          </div>
        </div>
      `;
      container.className = "devguide-content status-error";
      return;
    }

    // Render markdown to HTML
    try {
      const html = marked.parse(content);
      container.innerHTML = `<div class="devguide-markdown">${html}</div>`;
      container.className = "devguide-content";

      // Make external links open new windows
      // Bound to document so that we can handle dynamic content
      this._bindLinks();
    } catch (err) {
      container.innerHTML = `<pre>${this._escape(content)}</pre>`;
      container.className = "devguide-content";
    }
  }

  _escape(text) {
    return (text || "").replace(/[&<>"']/g, function (m) {
      return (
        {
          "&": "&amp;",
          "<": "&lt;",
          ">": "&gt;",
          '"': "&quot;",
          "'": "&#39;",
        }[m] || m
      );
    });
  }

  _bindLinks() {
    // Remove previous binding
    if (this._boundLinkHandler) {
      document.removeEventListener("click", this._boundLinkHandler);
      this._boundLinkHandler = null;
    }

    // Delegate to body: open external links in new window/tab
    this._boundLinkHandler = (e) => {
      const target = e.target.closest("a");
      if (!target) return;
      const href = target.getAttribute("href");
      if (!href || href.startsWith("#")) return; // ignore in-page anchors

      // If the link is relative to the doc or absolute outside this host, open in new tab
      if (
        href.startsWith("http://") ||
        href.startsWith("https://") ||
        href.startsWith("//") ||
        href.startsWith("/")
      ) {
        e.preventDefault();
        try {
          // Prefer core open method if available (native client)
          if (window.core && window.core.openExternal) {
            window.core.openExternal(href);
          } else {
            window.open(href, "_blank", "noopener");
          }
        } catch (err) {
          console.error("Failed to open external link:", href, err);
        }
      }
    };

    document.addEventListener("click", this._boundLinkHandler);
  }

  // Utility: copy text to clipboard (with fallback)
  _copyText(text) {
    if (!text) return Promise.reject(new Error("No text to copy"));
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text);
    }

    return new Promise((resolve, reject) => {
      try {
        const el = document.createElement("textarea");
        el.value = text;
        el.style.position = "fixed";
        el.style.left = "-9999px";
        el.style.top = "0";
        document.body.appendChild(el);
        el.focus();
        el.select();
        document.execCommand("copy");
        document.body.removeChild(el);
        resolve();
      } catch (err) {
        reject(err);
      }
    });
  }
}

export { DevGuidePage };
