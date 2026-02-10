//! Explain a specific finding category in detail

use anyhow::Result;
use colored::Colorize;

struct CategoryExplanation {
    prefix: &'static str,
    name: &'static str,
    description: &'static str,
    why_it_matters: &'static [&'static str],
    how_to_fix: &'static [&'static str],
    example_bad: &'static str,
    example_good: &'static str,
    references: &'static [&'static str],
}

const EXPLANATIONS: &[CategoryExplanation] = &[
    CategoryExplanation {
        prefix: "SEC",
        name: "Secret Exposure",
        description: "Hardcoded secrets, API keys, passwords, and tokens detected in source code. \
            These credentials should never be committed to version control, as they can be \
            extracted from git history even after deletion.",
        why_it_matters: &[
            "Secrets in source code are the #1 cause of credential leaks",
            "Git history preserves secrets forever, even after they are removed from HEAD",
            "Automated scanners continuously harvest secrets from public repositories",
            "A single leaked AWS key can cost thousands of dollars within minutes",
        ],
        how_to_fix: &[
            "Move secrets to environment variables or a secrets manager (e.g. AWS Secrets Manager, HashiCorp Vault)",
            "Use .env files locally and add .env to .gitignore",
            "If a secret was committed, rotate it immediately — removing the file is not enough",
            "Set up pre-commit hooks to prevent future secret commits",
        ],
        example_bad: r#"    const API_KEY = "AKIAIOSFODNN7EXAMPLE";"#,
        example_good: r#"    const API_KEY = std::env::var("AWS_ACCESS_KEY_ID")?;"#,
        references: &[
            "OWASP Secrets Management: https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html",
            "GitHub Secret Scanning: https://docs.github.com/en/code-security/secret-scanning",
        ],
    },
    CategoryExplanation {
        prefix: "SQL",
        name: "SQL Injection",
        description: "SQL injection via string interpolation, concatenation, or \
            formatting with user-controlled input. This allows attackers to execute arbitrary \
            SQL statements, potentially reading, modifying, or deleting data.",
        why_it_matters: &[
            "SQL injection is consistently in the OWASP Top 10 vulnerabilities",
            "A single vulnerable endpoint can expose the entire database",
            "Attackers can extract data, bypass authentication, or destroy tables",
            "Automated tools can discover and exploit SQL injection in seconds",
        ],
        how_to_fix: &[
            "Use parameterized queries / prepared statements with placeholders ($1, ?, :name)",
            "Use an ORM's query builder instead of raw SQL strings",
            "Never concatenate user input into SQL strings, even for column names or table names",
            "Apply input validation as defense-in-depth, but always parameterize",
        ],
        example_bad: r#"    db.query(`SELECT * FROM users WHERE id = '${userId}'`);"#,
        example_good: r#"    db.query("SELECT * FROM users WHERE id = $1", [userId]);"#,
        references: &[
            "OWASP SQL Injection: https://owasp.org/www-community/attacks/SQL_Injection",
            "CWE-89: Improper Neutralization of SQL: https://cwe.mitre.org/data/definitions/89.html",
        ],
    },
    CategoryExplanation {
        prefix: "ML",
        name: "ML Pipeline Anti-Pattern",
        description: "Common machine learning mistakes that lead to data leakage, \
            non-reproducible experiments, or unreliable models. These issues cause models \
            to perform well in testing but fail in production.",
        why_it_matters: &[
            "Data leakage inflates test metrics, hiding true model performance",
            "Non-reproducible splits make experiments impossible to compare fairly",
            "Pickle serialization is insecure and fragile across Python versions",
            "Hardcoded data paths break pipelines when deployed to different environments",
        ],
        how_to_fix: &[
            "Always fit preprocessing (scalers, encoders) on training data only, then transform test data",
            "Set random_state in train_test_split and model constructors for reproducibility",
            "Use joblib, ONNX, or MLflow for model serialization instead of pickle",
            "Use relative paths or environment variables for data locations",
        ],
        example_bad: r#"    scaler.fit(X_test)  # Data leakage!"#,
        example_good: r#"    scaler.fit(X_train)  # Fit on training data only"#,
        references: &[
            "Sklearn Pitfalls: https://scikit-learn.org/stable/common_pitfalls.html",
            "Data Leakage in ML: https://en.wikipedia.org/wiki/Leakage_(machine_learning)",
        ],
    },
    CategoryExplanation {
        prefix: "INFRA",
        name: "Infrastructure Misconfiguration",
        description: "Security misconfigurations in infrastructure-as-code (Terraform, \
            Kubernetes, Docker) that expose services to unauthorized access, data breaches, \
            or privilege escalation.",
        why_it_matters: &[
            "Misconfigured cloud resources are the leading cause of data breaches",
            "Public S3 buckets have exposed billions of records across major companies",
            "Overly permissive IAM policies violate the principle of least privilege",
            "Privileged containers can escape to the host, compromising the entire node",
        ],
        how_to_fix: &[
            "Set S3 bucket ACLs to private and use bucket policies for controlled access",
            "Restrict security group CIDR blocks to specific IP ranges, not 0.0.0.0/0",
            "Use specific IAM actions instead of wildcard (*) permissions",
            "Pin Docker image tags to specific versions, never use :latest in production",
        ],
        example_bad: r#"    acl = "public-read""#,
        example_good: r#"    acl = "private""#,
        references: &[
            "CIS AWS Benchmarks: https://www.cisecurity.org/benchmark/amazon_web_services",
            "OWASP Cloud Security: https://owasp.org/www-project-cloud-security/",
        ],
    },
    CategoryExplanation {
        prefix: "HOOKS",
        name: "React Hooks Anti-Pattern",
        description: "Violations of the Rules of Hooks and common React anti-patterns. \
            Hooks must be called at the top level of a component (not inside conditions, \
            loops, or nested functions) and must always be called in the same order.",
        why_it_matters: &[
            "Conditional hook calls break React's internal state tracking, causing crashes",
            "Missing dependency arrays cause useEffect to run every render, harming performance",
            "Direct DOM manipulation bypasses React's virtual DOM, leading to stale or lost state",
            "Missing key props in lists cause incorrect re-renders and subtle UI bugs",
        ],
        how_to_fix: &[
            "Always call hooks at the top level of your component, never inside conditions or loops",
            "Add a dependency array to useEffect: useEffect(() => { ... }, [dep1, dep2])",
            "Use useRef() instead of document.getElementById or querySelector",
            "Add a unique key prop when rendering lists: items.map(item => <Item key={item.id} />)",
        ],
        example_bad: r#"    if (isReady) useState(0);  // Hook inside condition"#,
        example_good: r#"    const [value, setValue] = useState(0);  // Always at top level"#,
        references: &[
            "Rules of Hooks: https://react.dev/reference/rules/rules-of-hooks",
            "useEffect: https://react.dev/reference/react/useEffect",
        ],
    },
    CategoryExplanation {
        prefix: "ASYNC",
        name: "Async Pattern Anti-Pattern",
        description: "Async/await misuse in JavaScript, TypeScript, and Python that causes \
            unhandled promise rejections, silent failures, race conditions, or floating \
            coroutines. These patterns compile and often appear to work, but fail under \
            load or error conditions.",
        why_it_matters: &[
            "Unhandled promise rejections crash Node.js processes in production",
            "forEach with async callbacks runs iterations in parallel without control",
            "Floating coroutines in Python silently do nothing — the work never executes",
            "Swallowed errors in .catch(() => {}) hide failures that should be investigated",
        ],
        how_to_fix: &[
            "Replace new Promise(async ...) with a plain async function or non-async executor",
            "Use for...of or Promise.all(items.map(...)) instead of .forEach(async ...)",
            "Always await asyncio calls in Python: await asyncio.sleep(1)",
            "Add .catch() to every .then() chain, or use async/await with try/catch",
        ],
        example_bad: r#"    items.forEach(async (item) => { await process(item); });"#,
        example_good: r#"    for (const item of items) { await process(item); }"#,
        references: &[
            "MDN Async/Await: https://developer.mozilla.org/en-US/docs/Learn/JavaScript/Asynchronous",
            "Python asyncio: https://docs.python.org/3/library/asyncio.html",
        ],
    },
    CategoryExplanation {
        prefix: "DEP",
        name: "Dependency Hygiene",
        description: "Import anti-patterns, deprecated modules, and manifest issues that \
            pollute namespaces, break builds, or make dependencies opaque. Covers wildcard \
            imports, circular dependency workarounds, unpinned versions, and git dependencies.",
        why_it_matters: &[
            "Wildcard imports pollute the namespace and make it impossible to trace where a name comes from",
            "Deprecated module imports will break when upgrading to newer Python versions",
            "Unpinned or wildcard dependency versions cause non-reproducible builds",
            "Git dependencies break offline installs and are harder to audit for vulnerabilities",
        ],
        how_to_fix: &[
            "Replace `from foo import *` with explicit imports: `from foo import bar, baz`",
            "Replace deprecated modules with their modern equivalents (e.g. argparse instead of optparse)",
            "Pin dependencies to specific versions or semver ranges in package.json",
            "Publish packages to a registry instead of depending on git URLs",
        ],
        example_bad: r#"    from utils import *"#,
        example_good: r#"    from utils import parse_config, validate_input"#,
        references: &[
            "PEP 8 Imports: https://peps.python.org/pep-0008/#imports",
            "Python 3.12 Removals: https://docs.python.org/3/whatsnew/3.12.html#removed",
        ],
    },
    CategoryExplanation {
        prefix: "IMPACT",
        name: "Change Impact",
        description: "Breaking or significant changes detected by comparing the current code \
            graph against the previous version. Revet tracks function signatures, class \
            structures, and dependency edges to identify changes that may affect callers \
            or downstream modules.",
        why_it_matters: &[
            "Changing a function signature can break every caller across the codebase",
            "Removing or renaming exports creates runtime errors in dependent modules",
            "Modifying class hierarchies can silently change behavior in subclasses",
            "Impact analysis catches issues that linters and type checkers may miss",
        ],
        how_to_fix: &[
            "Add default parameter values when extending function signatures",
            "Deprecate old APIs before removing them — provide migration paths",
            "Run revet with the full dependency graph to see all affected files",
            "Write tests for public API contracts to catch breaking changes early",
        ],
        example_bad: r#"    fn process(data: &str)  // was: fn process(data: &str, opts: Options)"#,
        example_good: r#"    fn process(data: &str, opts: Option<Options>)  // backward-compatible"#,
        references: &[
            "Semantic Versioning: https://semver.org/",
            "API Evolution: https://www.hyrumslaw.com/",
        ],
    },
    CategoryExplanation {
        prefix: "PARSE",
        name: "Parse Error",
        description: "Errors encountered while parsing source files into the code graph. \
            This usually indicates syntax errors, unsupported language constructs, or \
            files that could not be processed by the Tree-sitter grammar.",
        why_it_matters: &[
            "Unparseable files are excluded from impact analysis, creating blind spots",
            "Syntax errors in committed code will fail at build or runtime",
            "Parse errors may indicate corrupted files or encoding issues",
        ],
        how_to_fix: &[
            "Fix syntax errors in the reported file and line",
            "Ensure the file uses UTF-8 encoding",
            "Check that the language is supported by revet (Python, TypeScript, Go, Java)",
            "If the construct is valid but unsupported, file an issue at the revet repository",
        ],
        example_bad: r#"    def broken_function(  # missing closing paren and colon"#,
        example_good: r#"    def fixed_function():  # valid syntax"#,
        references: &[
            "Tree-sitter: https://tree-sitter.github.io/tree-sitter/",
            "Supported Languages: Python, TypeScript/JavaScript, Go, Java",
        ],
    },
];

fn extract_prefix(finding_id: &str) -> &str {
    finding_id.split('-').next().unwrap_or(finding_id)
}

fn get_explanation(prefix: &str) -> Option<&'static CategoryExplanation> {
    EXPLANATIONS.iter().find(|e| e.prefix == prefix)
}

fn print_explanation(explanation: &CategoryExplanation) {
    let separator = "\u{2501}".repeat(60);
    let thin_sep = "\u{2500}".repeat(55);

    println!();
    println!("  {}", separator.dimmed());
    println!(
        "  {} {} {}",
        explanation.prefix.bold().cyan(),
        "\u{2014}".dimmed(),
        explanation.name.bold()
    );
    println!("  {}", separator.dimmed());
    println!();
    println!("  {}", explanation.description);
    println!();

    println!("  {}", "Why It Matters:".bold());
    for point in explanation.why_it_matters {
        println!("    {} {}", "\u{2022}".dimmed(), point);
    }
    println!();

    println!("  {}", "How to Fix:".bold());
    for (i, step) in explanation.how_to_fix.iter().enumerate() {
        println!("    {}. {}", (i + 1).to_string().dimmed(), step);
    }
    println!();

    println!(
        "  {} {} {}",
        "\u{2500}\u{2500}".dimmed(),
        "Bad".red().bold(),
        thin_sep.dimmed()
    );
    println!("{}", explanation.example_bad.red());
    println!();

    println!(
        "  {} {} {}",
        "\u{2500}\u{2500}".dimmed(),
        "Good".green().bold(),
        thin_sep.dimmed()
    );
    println!("{}", explanation.example_good.green());
    println!();

    println!("  {}", "References:".bold());
    for reference in explanation.references {
        println!("    {} {}", "\u{2022}".dimmed(), reference.dimmed());
    }
    println!();
}

pub fn run(finding_id: &str, use_ai: bool) -> Result<()> {
    if use_ai {
        eprintln!(
            "{}",
            "  Note: LLM-powered explanations are not yet available. Showing standard explanation."
                .dimmed()
        );
        eprintln!();
    }

    let prefix = extract_prefix(finding_id);

    match get_explanation(prefix) {
        Some(explanation) => {
            print_explanation(explanation);
        }
        None => {
            eprintln!(
                "  {} Unknown finding prefix: {}",
                "Error:".red().bold(),
                prefix.yellow()
            );
            eprintln!();
            eprintln!("  Known prefixes:");
            for exp in EXPLANATIONS {
                eprintln!(
                    "    {} {} {}",
                    exp.prefix.cyan().bold(),
                    "\u{2014}".dimmed(),
                    exp.name
                );
            }
            eprintln!();
            eprintln!(
                "  Usage: {} {}",
                "revet explain".bold(),
                "<PREFIX-NNN>".dimmed()
            );
            eprintln!("  Example: {}", "revet explain SEC-001".bold());
            eprintln!();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_prefix_standard() {
        assert_eq!(extract_prefix("SEC-001"), "SEC");
        assert_eq!(extract_prefix("SQL-123"), "SQL");
        assert_eq!(extract_prefix("ML-042"), "ML");
    }

    #[test]
    fn test_extract_prefix_no_number() {
        assert_eq!(extract_prefix("SEC"), "SEC");
        assert_eq!(extract_prefix("IMPACT"), "IMPACT");
    }

    #[test]
    fn test_all_known_prefixes() {
        let known = [
            "SEC", "SQL", "ML", "INFRA", "HOOKS", "ASYNC", "DEP", "IMPACT", "PARSE",
        ];
        for prefix in &known {
            assert!(
                get_explanation(prefix).is_some(),
                "Missing explanation for prefix: {}",
                prefix
            );
        }
    }

    #[test]
    fn test_unknown_prefix() {
        assert!(get_explanation("CUSTOM").is_none());
        assert!(get_explanation("XYZ").is_none());
        assert!(get_explanation("").is_none());
    }

    #[test]
    fn test_explanation_has_content() {
        for exp in EXPLANATIONS {
            assert!(!exp.prefix.is_empty(), "prefix is empty");
            assert!(!exp.name.is_empty(), "name is empty for {}", exp.prefix);
            assert!(
                !exp.description.is_empty(),
                "description is empty for {}",
                exp.prefix
            );
            assert!(
                !exp.why_it_matters.is_empty(),
                "why_it_matters is empty for {}",
                exp.prefix
            );
            assert!(
                !exp.how_to_fix.is_empty(),
                "how_to_fix is empty for {}",
                exp.prefix
            );
            assert!(
                !exp.example_bad.is_empty(),
                "example_bad is empty for {}",
                exp.prefix
            );
            assert!(
                !exp.example_good.is_empty(),
                "example_good is empty for {}",
                exp.prefix
            );
            assert!(
                !exp.references.is_empty(),
                "references is empty for {}",
                exp.prefix
            );
        }
    }

    #[test]
    fn test_sec_explanation() {
        let exp = get_explanation("SEC").unwrap();
        assert!(
            exp.description.to_lowercase().contains("secret"),
            "SEC description should mention 'secret'"
        );
    }

    #[test]
    fn test_sql_explanation() {
        let exp = get_explanation("SQL").unwrap();
        assert!(
            exp.description.to_lowercase().contains("injection"),
            "SQL description should mention 'injection'"
        );
    }

    #[test]
    fn test_ml_explanation() {
        let exp = get_explanation("ML").unwrap();
        let desc = exp.description.to_lowercase();
        assert!(
            desc.contains("pipeline") || desc.contains("leakage"),
            "ML description should mention 'pipeline' or 'leakage'"
        );
    }
}
