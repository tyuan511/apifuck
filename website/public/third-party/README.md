# React Bits Aurora

ApiFuck's website hero adapts the Aurora vertex/fragment shaders and color conversion helpers by David Haz from React Bits. These third-party parts retain their original MIT + Commons Clause license; they are not relicensed under the repository's MIT license.

- Component: https://reactbits.dev/backgrounds/aurora
- Pinned upstream: https://github.com/DavidHDev/react-bits/blob/43591e8b3fa327d45633b4e8239ff6742ef706ef/src/content/Backgrounds/Aurora/Aurora.jsx
- Upstream license: https://github.com/DavidHDev/react-bits/blob/43591e8b3fa327d45633b4e8239ff6742ef706ef/LICENSE.md
- Full notice: [react-bits-LICENSE.txt](react-bits-LICENSE.txt)
- Local integration: `website/src/hero-aurora.js`.

Changes: native browser lifecycle instead of React; ApiFuck blue/cyan/indigo palette; bounded render resolution and frame rate; static frames for reduced motion and pause; offscreen/background suspension, context-loss recovery, and a separate animated CSS light-ribbon fallback (static when motion is paused or reduced). Included only as part of this website.
