# English (en-US) Fluent translations for repository-related messages.
# IDs correspond to dotted keys with '.' and '_' replaced by '-' (see dotted_to_fluent_id).

# repo.result_line_simple
# Arg0: name
# Arg1: short description
# Arg2: score suffix (e.g., "(0.87)") — may be empty
repo-result-line-simple = { $arg0 } — { $arg1 } { $arg2 }

# repo.score_format
# Arg0: numeric score (e.g. 0.87) -> outputs "(0.87)"
repo-score-format = ({ $arg0 })

# repo.no_results_for
# Arg0: query string
repo-no-results-for = No results found for "{ $arg0 }".

# Labels
repo-authors = Authors
repo-url = URL
repo-version = Version
repo-updated = Updated

# Download / asset related messages
repo-no-downloadable-zip-asset = No downloadable ZIP asset found for this release.
repo-confirm-download = Do you want to download "{ $arg0 }"?
repo-skipped-download = Download skipped: { $arg0 }
repo-saved = Saved to { $arg0 }.
repo-failed-to-download = Failed to download: { $arg0 }.

# Search / query
repo-search-empty-query = Search query cannot be empty.

# Summary / status messages
# Arg0: number of updated modules
repo-updated-modules = Updated { $arg0 } module(s)
