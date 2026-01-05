# C06 — clippy api remains 修复记录（20251231-1651）

## 修改的函数签名列表

### `src/cmds/secret/handler.rs`

- `fn handle_interactive_add_direct(name: String) -> Result<(), KamError>`
  - 改为：`fn handle_interactive_add_direct(name: &str) -> Result<(), KamError>`

- `fn handle_interactive_add_file(name: String) -> Result<(), KamError>`
  - 改为：`fn handle_interactive_add_file(name: &str) -> Result<(), KamError>`

- `fn handle_add(name: String, file: Option<PathBuf>, file_path: Option<PathBuf>, value: Option<String>, force_file: bool, password: Option<String>, _with_backup: bool) -> Result<(), KamError>`
  - 改为：`fn handle_add(name: &str, file: Option<&PathBuf>, file_path: Option<&PathBuf>, value: Option<&str>, force_file: bool, password: Option<&str>, _with_backup: bool) -> Result<(), KamError>`

- `fn handle_get(name: String, out: Option<PathBuf>, password: Option<String>) -> Result<(), KamError>`
  - 改为：`fn handle_get(name: &str, out: Option<&PathBuf>, password: Option<&str>) -> Result<(), KamError>`

- `fn handle_remove(name: String) -> Result<(), KamError>`
  - 改为：`fn handle_remove(name: &str) -> Result<(), KamError>`

- `fn handle_export(name: String, path: PathBuf, encrypted: bool) -> Result<(), KamError>`
  - 改为：`fn handle_export(name: &str, path: &PathBuf, encrypted: bool) -> Result<(), KamError>`

- `fn handle_import(path: PathBuf, name: Option<String>) -> Result<(), KamError>`
  - 改为：`fn handle_import(path: &PathBuf, name: Option<String>) -> Result<(), KamError>`

- `fn handle_export_pub(name: String, out: Option<PathBuf>) -> Result<(), KamError>`
  - 改为：`fn handle_export_pub(name: &str, out: Option<PathBuf>) -> Result<(), KamError>`

- `fn handle_import_cert(repo: Option<String>, issue: Option<u32>, cert_chain: Option<PathBuf>, name: String) -> Result<(), KamError>`
  - 改为：`fn handle_import_cert(repo: Option<String>, issue: Option<u32>, cert_chain: Option<&PathBuf>, name: &str) -> Result<(), KamError>`
