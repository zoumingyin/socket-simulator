/**
 * 浏览器控制台调试脚本 - 检查 Socket.IO 连接状态
 * 使用方法：在浏览器控制台（F12）粘贴并运行此脚本
 */
(function debugSocketConnection() {
  console.log('====== Socket.IO 连接调试开始 ======');
  
  // 1. 检查 socket.io-client 是否可用
  console.log('[1] 检查 socket.io-client 是否可用...');
  if (typeof io !== 'undefined') {
    console.log('✅ socket.io-client 已加载');
  } else {
    console.error('❌ socket.io-client 未加载，请检查 index.html 是否引入了 socket.io-client');
  }
  
  // 2. 手动连接 Socket.IO 管理通道
  console.log('[2] 手动连接 Socket.IO 管理通道...');
  const socket = io('http://localhost:3080', {
    path: '/admin/socket.io',
    transports: ['websocket', 'polling'],
    reconnection: true,
    timeout: 10000,
  });
  
  socket.on('connect', () => {
    console.log('✅ [手动测试] Socket 连接成功!');
    console.log('   socket.id:', socket.id);
    console.log('   现在应该能收到 runtime_update 事件了...');
  });
  
  socket.on('runtime_update', (runtimes) => {
    console.log('✅ [手动测试] 收到 runtime_update 事件!');
    console.log('   数据:', runtimes);
    console.log('   现在请发送一条消息，看是否能实时收到更新...');
  });
  
  socket.on('connect_error', (err) => {
    console.error('❌ [手动测试] Socket 连接失败:', err.message);
    console.error('   可能的原因:');
    console.error('   1. 后端未启动（端口 3080）');
    console.error('   2. 后端 Socket.IO path 配置错误');
    console.error('   3. 跨域问题（CORS）');
  });
  
  socket.on('disconnect', (reason) => {
    console.log('❌ [手动测试] Socket 断开:', reason);
  });
  
  // 3. 检查 Zustand Store 中的 adminSocket 状态
  console.log('[3] 检查 Zustand Store 中的 adminSocket 状态...');
  setTimeout(() => {
    // 尝试从 window 获取 store（如果导出了）
    if (window.__SERVE_STORE__) {
      const state = window.__SERVE_STORE__.getState();
      console.log('   adminSocket:', state.adminSocket);
      console.log('   adminSocket?.connected:', state.adminSocket?.connected);
    } else {
      console.log('   ⚠️ 无法访问 Zustand Store，请在前端代码中导出 store');
    }
  }, 2000);
  
  console.log('====== Socket.IO 连接调试结束 ======');
  console.log('请观察上面的日志，看是否有 [手动测试] 相关输出');
})();
