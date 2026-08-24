# 栖语 React Web

`web2` 是独立于 Vue 客户端的 React SPA，使用 React 19、React Router、Axios、Ant Design 和 Tailwind CSS。

## 本地开发

后端默认监听 `127.0.0.1:3000`，Vite 会代理 `/api` 和 `/ws`：

```bash
cd web2
bun install
bun run dev
```

生产构建：

```bash
bun run lint
bun run build
```

## Cargo 打包选择

从仓库根目录选择要嵌入 Rust 二进制的前端：

```bash
cargo build --features react
cargo build --features vue
```

不传 feature 时仍打包 Vue，以兼容原有构建命令。`react` 与 `vue` 不能同时启用。
