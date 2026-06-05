// Shared flat ESLint config base for Howllo TypeScript/React apps.
// Apps extend this and add framework-specific plugins (e.g. next, vite).
export default [
  {
    rules: {
      "no-unused-vars": "off",
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
    },
  },
];
