import antfu from '@antfu/eslint-config'

export default antfu({
  ignores: [
    '.claude/**',
    'src-tauri/**',
  ],
  rules: {
    'node/prefer-global/process': 'off',
  },
})
