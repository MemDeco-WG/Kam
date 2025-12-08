import { defineConfig } from 'vite'
import { glob } from 'glob'
import path from 'path'

export default defineConfig({
  // 设置为相对路径，这样build后的资源路径就是相对的
  base: './',
  
  build: {
    // 输出目录
    outDir: 'dist',
    // 静态资源目录
    assetsDir: 'assets',
    // 生成sourcemap用于调试
    sourcemap: false,
    // 压缩代码
    minify: 'esbuild',
    // 设置chunk大小警告限制
    chunkSizeWarningLimit: 1000,
    // 配置rollup选项来处理CSS文件
    rollupOptions: {
      input: {
        main: path.resolve(__dirname, 'index.html'),
        // 添加页面CSS文件作为入口
        ...Object.fromEntries(
          glob.sync('src/assets/css/pages/*.css').map(file => [
            path.basename(file, '.css'),
            path.resolve(__dirname, file)
          ])
        )
      },
      output: {
        // 保持CSS文件结构
        assetFileNames: (assetInfo) => {
          if (assetInfo.name && assetInfo.name.endsWith('.css')) {
            // 如果是页面CSS文件，保持在pages目录下
            const cssFiles = glob.sync('src/assets/css/pages/*.css')
            const isPageCSS = cssFiles.some(file => 
              path.basename(file, '.css') === path.basename(assetInfo.name, '.css')
            )
            if (isPageCSS) {
              return 'assets/css/pages/[name][extname]'
            }
          }
          return 'assets/[name]-[hash][extname]'
        }
      }
    }
  },
  
  server: {
    // 开发服务器端口
    port: 5174,
    // 自动打开浏览器
    open: false,
    // 允许外部访问
    host: true
  }
})