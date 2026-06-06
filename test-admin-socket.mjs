/**
 * 测试脚本 - 连接后端 Socket.IO 管理通道
 * 用于排查前端收不到实时推送的问题
 */
import { io } from 'socket.io-client';

const socket = io('http://localhost:3080', {
  path: '/admin/socket.io',
  transports: ['websocket'],  // 仅使用 WebSocket，禁用 polling
  reconnection: true,
  timeout: 10000,
});

console.log('[测试客户端] 开始连接...');
console.log('[测试客户端] 配置:', {
  url: 'http://localhost:3080',
  path: '/admin/socket.io',
  transports: ['websocket'],
});

socket.on('connect', () => {
  console.log('[测试客户端] ✅ 连接成功!');
  console.log('[测试客户端] socket.id:', socket.id);
  console.log('[测试客户端] 等待接收 runtime_update 事件...');
  console.log('[测试客户端] 如果 10 秒内未收到消息，将输出诊断信息\n');
});

socket.on('runtime_update', (runtimes) => {
  console.log('[测试客户端] ✅ 收到 runtime_update 事件!');
  console.log('[测试客户端] 数据:', JSON.stringify(runtimes, null, 2));
  console.log('');
});

socket.on('disconnect', (reason) => {
  console.log('[测试客户端] ❌ 连接断开:', reason);
});

socket.on('connect_error', (err) => {
  console.error('[测试客户端] ❌ 连接失败:', err.message);
  console.error('[测试客户端] 错误详情:', err);
});

socket.on('reconnect', (attempt) => {
  console.log('[测试客户端] 🔄 重连成功，尝试次数:', attempt);
});

socket.on('reconnect_error', (err) => {
  console.error('[测试客户端] 🔄 重连失败:', err.message);
});

// 10 秒后未收到消息，输出诊断信息
setTimeout(() => {
  console.log('\n[测试客户端] ⏰ 10 秒超时诊断:');
  console.log('[测试客户端] 连接状态:', socket.connected ? '已连接' : '未连接');
  console.log('[测试客户端] socket.id:', socket.id || '无');
  console.log('[测试客户端] 可能的原因:');
  console.log('  1. 后端未正确推送 runtime_update 事件');
  console.log('  2. 前端与后端 Socket.IO 版本不兼容');
  console.log('  3. 事件名不匹配（后端推送的事件名与前端监听的不一致）');
  console.log('  4. 后端 ServiceManager 的 runtime_updated 事件未正确触发');
  console.log('\n[测试客户端] 建议:');
  console.log('  1. 检查后端控制台是否有 "[AdminSocket] 管理界面已连接" 日志');
  console.log('  2. 检查后端 ServiceManager.updateRuntime() 是否被正确调用');
  console.log('  3. 检查后端是否调用了 serviceManager.emit("runtime_updated", ...)');
}, 10000);

// 20 秒后退出
setTimeout(() => {
  console.log('\n[测试客户端] 测试结束，断开连接');
  socket.disconnect();
  process.exit(0);
}, 20000);
