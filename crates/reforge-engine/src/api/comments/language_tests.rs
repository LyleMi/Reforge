use super::tests::TestWorkspace;
use super::*;
use std::fs;

#[test]
fn common_languages_remove_comments_and_preserve_literal_delimiters() {
    let cases = [
        ("a.dart", "// remove\nvar x = '// literal';\n", "// literal"),
        ("a.py", "# remove\nx = '# literal'\n", "# literal"),
        (
            "a.go",
            "package main\n// remove\nvar x = `// literal`\n",
            "// literal",
        ),
        (
            "A.java",
            "// remove\nclass A { String x = \"// literal\"; }\n",
            "// literal",
        ),
        (
            "a.cs",
            "// remove\nclass A { string x = @\"// literal\"; }\n",
            "// literal",
        ),
        ("a.kt", "// remove\nval x = \"// literal\"\n", "// literal"),
        (
            "a.php",
            "<?php\n# remove\n$x = '// literal';\n",
            "// literal",
        ),
        ("a.rb", "# remove\nx = '# literal'\n", "# literal"),
        ("a.sh", "# remove\nx='# literal'\n", "# literal"),
        ("a.ps1", "# remove\n$x = '# literal'\n", "# literal"),
        (
            "a.c",
            "// remove\nconst char *x = \"// literal\";\n",
            "// literal",
        ),
        (
            "a.cpp",
            "/* remove */\nauto x = R\"(// literal)\";\n",
            "// literal",
        ),
        (
            "a.swift",
            "// remove\nlet x = \"// literal\"\n",
            "// literal",
        ),
        ("a.lua", "-- remove\nx = [=[-- literal]=]\n", "-- literal"),
        (
            "a.scala",
            "// remove\nval x = \"// literal\"\n",
            "// literal",
        ),
        ("a.R", "# remove\nx <- '# literal'\n", "# literal"),
        (
            "a.html",
            "<!-- remove -->\n<div title=\"<!-- literal -->\">x</div>\n",
            "<!-- literal -->",
        ),
        (
            "a.css",
            "/* remove */\na { content: '/* literal */'; }\n",
            "/* literal */",
        ),
        (
            "a.jsonc",
            "// remove\n{\"x\": \"// literal\"}\n",
            "// literal",
        ),
        ("a.yaml", "# remove\nx: '# literal'\n", "# literal"),
        ("a.toml", "# remove\nx = '# literal'\n", "# literal"),
        ("a.sql", "-- remove\nSELECT '-- literal';\n", "-- literal"),
    ];
    for (path, source, literal) in cases {
        let workspace = TestWorkspace::new();
        workspace.write(path, source);
        let inventory = workspace.pick();
        assert!(
            inventory.skipped.is_empty(),
            "{path}: {:?}",
            inventory.skipped
        );
        assert_eq!(inventory.comments.len(), 1, "{path}");
        assert_eq!(
            extract::body(&inventory.comments[0].text),
            "remove",
            "{path}"
        );
        let plan = prepare_all(&workspace.0, &Config::defaults())
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(plan.files.len(), 1, "{path}");
        let saved: CleanPlan =
            serde_json::from_str(&serde_json::to_string(&plan).unwrap()).unwrap();
        apply(&saved).unwrap_or_else(|error| panic!("{path}: {error}"));
        let cleaned = fs::read_to_string(workspace.0.join(path)).unwrap();
        assert!(cleaned.contains(literal), "{path}: {cleaned}");
        assert!(!cleaned.contains("remove"), "{path}: {cleaned}");
        assert!(workspace.pick().comments.is_empty(), "{path}");
    }
}

#[test]
fn recognizes_block_bodies_and_exact_selectors_across_languages() {
    for (path, source) in [
        ("a.lua", "--[==[remove]==]\nx = 1\n"),
        ("a.rb", "=begin\nremove\n=end\nx = 1\n"),
        ("a.ps1", "<# remove #>\n$x = 1\n"),
        ("a.sql", "/* remove */\nSELECT 1;\n"),
    ] {
        let workspace = TestWorkspace::new();
        workspace.write(path, source);
        let selection = Selection {
            text: vec!["remove".into()],
            kind: Some(CommentKind::Block),
            ..Default::default()
        };
        let plan = prepare(&workspace.0, &Config::defaults(), &selection).unwrap();
        assert_eq!(plan.selected_comments, 1, "{path}");
        apply(&plan).unwrap();
        assert!(workspace.pick().comments.is_empty(), "{path}");
    }
}

#[test]
fn preserves_docstrings_heredocs_and_sql_comment_statements() {
    for (path, source) in [
        (
            "a.py",
            "\"\"\"# module documentation\"\"\"\n# remove\nx = 1\n",
        ),
        ("a.sh", "cat <<'EOF'\n# literal\nEOF\n# remove\n"),
        ("a.rb", "x = <<~TEXT\n# literal\nTEXT\n# remove\n"),
        (
            "a.sql",
            "COMMENT ON TABLE users IS 'description';\n-- remove\n",
        ),
        ("a.yaml", "x: |\n  # literal\n# remove\n"),
        (
            "a.html",
            "<script>const x = '// literal'; // retained</script>\n<!-- remove -->\n",
        ),
    ] {
        let workspace = TestWorkspace::new();
        workspace.write(path, source);
        let inventory = workspace.pick();
        assert!(
            inventory.skipped.is_empty(),
            "{path}: {:?}",
            inventory.skipped
        );
        assert_eq!(
            inventory.comments.len(),
            1,
            "{path}: {:?}",
            inventory.comments
        );
        let plan = prepare_all(&workspace.0, &Config::defaults()).unwrap();
        apply(&plan).unwrap();
        assert_eq!(
            fs::read_to_string(workspace.0.join(path)).unwrap(),
            source
                .replace("# remove\n", "")
                .replace("-- remove\n", "")
                .replace("<!-- remove -->\n", ""),
            "{path}"
        );
    }
}

#[test]
fn selected_cleanup_protects_cross_language_tool_directives() {
    for (path, source) in [
        ("a.py", "# coding: utf-8\nx = 1 # noqa\n"),
        ("a.go", "//go:build linux\n\npackage main\n"),
        (
            "a.sh",
            "#!/bin/bash\n# shellcheck disable=SC2086\necho hi\n",
        ),
        ("a.rb", "# frozen_string_literal: true\nx = 1\n"),
        ("a.ps1", "#requires -Version 7\n$x = 1\n"),
        ("a.cpp", "// clang-format off\nint x;\n"),
    ] {
        let workspace = TestWorkspace::new();
        workspace.write(path, source);
        let inventory = workspace.pick();
        assert!(
            inventory.skipped.is_empty(),
            "{path}: {:?}",
            inventory.skipped
        );
        assert!(!inventory.comments.is_empty(), "{path}");
        assert!(
            inventory.comments.iter().all(|comment| comment.protected),
            "{path}: {:?}",
            inventory.comments
        );
        let plan = prepare(
            &workspace.0,
            &Config::defaults(),
            &Selection {
                ids: inventory
                    .comments
                    .iter()
                    .map(|comment| comment.id.clone())
                    .collect(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(plan.files.is_empty(), "{path}");
    }
}

#[test]
fn removes_comment_only_files_across_grammars() {
    for (path, source) in [
        ("a.py", "# remove\n"),
        ("a.yaml", "# remove\n"),
        ("a.toml", "# remove\n"),
        ("a.html", "<!-- remove -->\n"),
        ("a.lua", "-- remove\n"),
        ("a.sql", "-- remove\n"),
    ] {
        let workspace = TestWorkspace::new();
        workspace.write(path, source);
        let plan = prepare_all(&workspace.0, &Config::defaults())
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(plan.files.len(), 1, "{path}");
        apply(&plan).unwrap();
        assert_eq!(
            fs::read_to_string(workspace.0.join(path)).unwrap(),
            "",
            "{path}"
        );
    }
}
