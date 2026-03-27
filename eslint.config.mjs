import antfu from '@antfu/eslint-config'

export default antfu({
  ignores: [
    '.agents/**',
    'src-tauri/**',
  ],
  rules: {
    'node/prefer-global/process': 'off',
  },
})
