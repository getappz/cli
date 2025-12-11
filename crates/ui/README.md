# UI Crate

UI/UX components for the CLI application.

## Components

### Tables
Display structured data with consistent formatting.

```rust
use ui::table;

let data = vec![
    vec!["ID", "Status", "Created"],
    vec!["123", "active", "2024-01-01"],
];
table::display(&data)?;
```

### Lists
Grouped and hierarchical list formatting.

```rust
use ui::list;

list::display_grouped(&groups)?;
```

### Status Messages
Success, error, warning, and info indicators.

```rust
use ui::status;

status::success("Operation completed!")?;
status::error("Something went wrong")?;
status::warning("This may take a while")?;
status::info("Processing...")?;
```

### Progress Indicators
Spinners and progress bars for async operations.

```rust
use ui::progress;

let spinner = progress::spinner("Loading...");
// ... async work ...
spinner.finish();
```

### Formatters
Date/time, status badges, and number formatting.

```rust
use ui::format;

let date = format::timestamp(1234567890)?;
let badge = format::status_badge("active")?;
```

### Prompts
User input helpers.

```rust
use ui::{prompt, confirm, choose};

let name = prompt("Enter your name:", None)?;
let yes = confirm("Continue?", true)?;
let choice = choose("Select option:", &["A", "B", "C"], None)?;
```

### Empty States
Consistent "no data" messages.

```rust
use ui::empty;

empty::display("No deployments found", Some("Try creating a deployment first"))?;
```

### Error Display
Formatted error messages.

```rust
use ui::error;

error::display("Failed to connect", &err)?;
```

### Pagination
Display pagination information.

```rust
use ui::pagination;

pagination::display(&pagination_info)?;
```

