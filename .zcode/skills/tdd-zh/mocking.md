# 何时 Mock

只 mock **系统边界**：

- 外部 API（支付、邮件等）
- 数据库（有时——优先用 test DB）
- 时间/随机性
- 文件系统（有时）

不要 mock：

- 你自己的类/模块
- 内部协作者
- 你控制的任何东西

## 为可 Mock 而设计

在系统边界，设计易于 mock 的接口：

**1. 用依赖注入**

把外部依赖传进来，而不是在内部创建：

```typescript
// 容易 mock
function processPayment(order, paymentClient) {
  return paymentClient.charge(order.total);
}

// 难 mock
function processPayment(order) {
  const client = new StripeClient(process.env.STRIPE_KEY);
  return client.charge(order.total);
}
```

**2. SDK 形态的接口优先于通用 fetcher**

为每个外部操作写专门的函数，而不是一个带条件逻辑的通用函数：

```typescript
// 好：每个函数独立可 mock
const api = {
  getUser: (id) => fetch(`/users/${id}`),
  getOrders: (userId) => fetch(`/users/${userId}/orders`),
  createOrder: (data) => fetch('/orders', { method: 'POST', body: data }),
};

// 坏：mock 需要条件逻辑
const api = {
  fetch: (endpoint, options) => fetch(endpoint, options),
};
```

SDK 做法意味着：

- 每个 mock 返回一种特定形态
- 测试 setup 没有条件逻辑
- 更容易看出测试覆盖了哪些端点
- 逐端点的类型安全