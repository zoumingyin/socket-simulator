// test-websocket-client.cjs - 原生 WebSocket 测试客户端
const WebSocket = require('ws');

const ws = new WebSocket('ws://localhost:1111');
let msgCount = 0;

ws.on('open', () => {
  console.log(`[${new Date().toISOString()}] ✅ WS连接成功! socket.id=${ws.url}`);
});

ws.on('message', (data) => {
  msgCount++;
  const str = data.toString();
  console.log(`[${new Date().toISOString()}] 📨 收到消息 #${msgCount}: ${str.substring(0, 200)}`);
});

ws.on('error', (err) => {
  console.error(`[${new Date().toISOString()}] ❌ WS错误:`, err.message);
});

ws.on('close', () => {
  console.log(`[${new Date().toISOString()}] 🔌 WS连接关闭，共收到 ${msgCount} 条消息`);
});

// 15秒后自动退出
setTimeout(() => {
  console.log(`[${new Date().toISOString()}] ⏱️ 测试结束，共收到 ${msgCount} 条消息`);
  ws.close();
  process.exit(0);
}, 15000);
