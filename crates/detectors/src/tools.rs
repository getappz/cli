//! Tool and library detection for skills recommendations.

use crate::filesystem::DetectorFilesystem;
use std::collections::HashMap;
use std::sync::Arc;

struct ToolPattern {
    name: &'static str,
    config_files: &'static [&'static str],
    files: &'static [&'static str],
    dependencies: &'static [&'static str],
}

const TOOL_PATTERNS: &[ToolPattern] = &[
    ToolPattern {
        name: "prisma",
        config_files: &["prisma/schema.prisma"],
        files: &[],
        dependencies: &["prisma", "@prisma/client"],
    },
    ToolPattern {
        name: "drizzle",
        config_files: &["drizzle.config.ts", "drizzle.config.js"],
        files: &[],
        dependencies: &["drizzle-orm"],
    },
    ToolPattern {
        name: "typeorm",
        config_files: &[],
        files: &[],
        dependencies: &["typeorm"],
    },
    ToolPattern {
        name: "sequelize",
        config_files: &[],
        files: &[],
        dependencies: &["sequelize"],
    },
    ToolPattern {
        name: "mongoose",
        config_files: &[],
        files: &[],
        dependencies: &["mongoose"],
    },
    ToolPattern {
        name: "kysely",
        config_files: &[],
        files: &[],
        dependencies: &["kysely"],
    },
    ToolPattern {
        name: "tailwind",
        config_files: &[
            "tailwind.config.js",
            "tailwind.config.ts",
            "tailwind.config.mjs",
            "tailwind.config.cjs",
        ],
        files: &[],
        dependencies: &["tailwindcss"],
    },
    ToolPattern {
        name: "styled-components",
        config_files: &[],
        files: &[],
        dependencies: &["styled-components"],
    },
    ToolPattern {
        name: "emotion",
        config_files: &[],
        files: &[],
        dependencies: &["@emotion/react", "@emotion/styled"],
    },
    ToolPattern {
        name: "sass",
        config_files: &[],
        files: &[],
        dependencies: &["sass", "node-sass"],
    },
    ToolPattern {
        name: "less",
        config_files: &[],
        files: &[],
        dependencies: &["less"],
    },
    ToolPattern {
        name: "webpack",
        config_files: &["webpack.config.js", "webpack.config.ts"],
        files: &[],
        dependencies: &["webpack"],
    },
    ToolPattern {
        name: "esbuild",
        config_files: &[],
        files: &[],
        dependencies: &["esbuild"],
    },
    ToolPattern {
        name: "rollup",
        config_files: &["rollup.config.js", "rollup.config.ts"],
        files: &[],
        dependencies: &["rollup"],
    },
    ToolPattern {
        name: "turbopack",
        config_files: &[],
        files: &[],
        dependencies: &["@serwist/turbopack", "@vercel/turbopack", "turbopack"],
    },
    ToolPattern {
        name: "turborepo",
        config_files: &["turbo.json"],
        files: &[],
        dependencies: &["turbo"],
    },
    ToolPattern {
        name: "redux",
        config_files: &[],
        files: &[],
        dependencies: &["redux", "@reduxjs/toolkit"],
    },
    ToolPattern {
        name: "zustand",
        config_files: &[],
        files: &[],
        dependencies: &["zustand"],
    },
    ToolPattern {
        name: "jotai",
        config_files: &[],
        files: &[],
        dependencies: &["jotai"],
    },
    ToolPattern {
        name: "recoil",
        config_files: &[],
        files: &[],
        dependencies: &["recoil"],
    },
    ToolPattern {
        name: "mobx",
        config_files: &[],
        files: &[],
        dependencies: &["mobx"],
    },
    ToolPattern {
        name: "graphql",
        config_files: &[],
        files: &[],
        dependencies: &["graphql", "@apollo/client", "urql"],
    },
    ToolPattern {
        name: "trpc",
        config_files: &[],
        files: &[],
        dependencies: &["@trpc/server", "@trpc/client"],
    },
    ToolPattern {
        name: "tanstack-query",
        config_files: &[],
        files: &[],
        dependencies: &["@tanstack/react-query", "react-query"],
    },
    ToolPattern {
        name: "swr",
        config_files: &[],
        files: &[],
        dependencies: &["swr"],
    },
    ToolPattern {
        name: "axios",
        config_files: &[],
        files: &[],
        dependencies: &["axios"],
    },
    ToolPattern {
        name: "nextauth",
        config_files: &[],
        files: &[],
        dependencies: &["next-auth"],
    },
    ToolPattern {
        name: "clerk",
        config_files: &[],
        files: &[],
        dependencies: &["@clerk/nextjs", "@clerk/clerk-react"],
    },
    ToolPattern {
        name: "auth0",
        config_files: &[],
        files: &[],
        dependencies: &["@auth0/nextjs-auth0", "@auth0/auth0-react"],
    },
    ToolPattern {
        name: "supabase",
        config_files: &[],
        files: &[],
        dependencies: &["@supabase/supabase-js", "@supabase/auth-helpers-nextjs"],
    },
    ToolPattern {
        name: "firebase",
        config_files: &[],
        files: &[],
        dependencies: &["firebase", "firebase-admin"],
    },
    ToolPattern {
        name: "docker",
        config_files: &["Dockerfile", "docker-compose.yml", "docker-compose.yaml"],
        files: &[],
        dependencies: &[],
    },
    ToolPattern {
        name: "kubernetes",
        config_files: &[],
        files: &["k8s", "kubernetes", "helm"],
        dependencies: &[],
    },
    ToolPattern {
        name: "terraform",
        config_files: &[],
        files: &["main.tf", "terraform"],
        dependencies: &[],
    },
    ToolPattern {
        name: "pulumi",
        config_files: &["Pulumi.yaml"],
        files: &[],
        dependencies: &[],
    },
    ToolPattern {
        name: "eslint",
        config_files: &[".eslintrc", ".eslintrc.js", ".eslintrc.json", "eslint.config.js"],
        files: &[],
        dependencies: &["eslint"],
    },
    ToolPattern {
        name: "prettier",
        config_files: &[".prettierrc", ".prettierrc.js", ".prettierrc.json", "prettier.config.js"],
        files: &[],
        dependencies: &["prettier"],
    },
    ToolPattern {
        name: "biome",
        config_files: &["biome.json", "biome.jsonc"],
        files: &[],
        dependencies: &["@biomejs/biome"],
    },
    ToolPattern {
        name: "nx",
        config_files: &["nx.json"],
        files: &[],
        dependencies: &["nx"],
    },
    ToolPattern {
        name: "lerna",
        config_files: &["lerna.json"],
        files: &[],
        dependencies: &["lerna"],
    },
    ToolPattern {
        name: "changesets",
        config_files: &[],
        files: &[".changeset"],
        dependencies: &["@changesets/cli"],
    },
    ToolPattern {
        name: "storybook",
        config_files: &[],
        files: &[".storybook"],
        dependencies: &["@storybook/react", "storybook"],
    },
    ToolPattern {
        name: "docusaurus",
        config_files: &[],
        files: &[],
        dependencies: &["@docusaurus/core"],
    },
    ToolPattern {
        name: "openai",
        config_files: &[],
        files: &[],
        dependencies: &["openai"],
    },
    ToolPattern {
        name: "anthropic",
        config_files: &[],
        files: &[],
        dependencies: &["@anthropic-ai/sdk"],
    },
    ToolPattern {
        name: "langchain",
        config_files: &[],
        files: &[],
        dependencies: &["langchain", "@langchain/core"],
    },
    ToolPattern {
        name: "vercel-ai",
        config_files: &[],
        files: &[],
        dependencies: &["ai"],
    },
];

/// Detect tools and libraries in the project.
pub async fn detect_tools(
    fs: &Arc<dyn DetectorFilesystem>,
    all_dependencies: &HashMap<String, String>,
) -> Vec<String> {
    let mut detected = Vec::new();

    for pattern in TOOL_PATTERNS {
        if matches_tool_pattern(fs, all_dependencies, pattern).await {
            detected.push(pattern.name.to_string());
        }
    }

    // Exclude superseded tools
    if detected.contains(&"turbopack".to_string()) && detected.contains(&"webpack".to_string()) {
        detected.retain(|t| t != "webpack");
    }
    if detected.contains(&"biome".to_string()) {
        detected.retain(|t| t != "eslint" && t != "prettier");
    }

    detected
}

async fn matches_tool_pattern(
    fs: &Arc<dyn DetectorFilesystem>,
    all_dependencies: &HashMap<String, String>,
    pattern: &ToolPattern,
) -> bool {
    for file in pattern.config_files {
        if fs.has_path(file).await {
            return true;
        }
    }

    for file in pattern.files {
        if fs.has_path(file).await {
            return true;
        }
    }

    for dep in pattern.dependencies {
        if all_dependencies.contains_key(*dep) {
            return true;
        }
    }

    false
}
