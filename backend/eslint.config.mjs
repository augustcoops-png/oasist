import js from "@eslint/js";
import globals from "globals";

export default [
  js.configs.recommended,
  // Source files
  {
    files: ["**/*.js"],
    ignores: ["**/__tests__/**"],
    languageOptions: {
      ecmaVersion: 2022,
      globals: {
        ...globals.node,
      },
    },
    rules: {
      "no-console": "off",
    },
  },
  // Test files — add Jest globals
  {
    files: ["**/__tests__/**/*.js", "**/*.test.js"],
    languageOptions: {
      ecmaVersion: 2022,
      globals: {
        ...globals.node,
        ...globals.jest,
      },
    },
    rules: {
      "no-console": "off",
    },
  },
];
