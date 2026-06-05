const io = require('socket.io-client');
const socket = io('http://localhost:1111');

socket.on('connect', () => {
  console.log('[客户端] 已连接，ID:', socket.id);
});

socket.on('test_poll', (data) => {
  console.log('[客户端] 收到轮询消息:', JSON.stringify(data));
});

socket.on('connect_error', (err) => {
  console.error('[客户端] 连接失败:', err.message);
});

// 20秒后自动退出
setTimeout(() => {
  console.log('[客户端] 测试结束，断开连接');
  socket.disconnect();
  process.exit(0);
}, 20000);
