# 好测试与坏测试

## 好测试

**集成式**：通过真接口测，不通过内部部件的 mock。

```typescript
// 好：测可观察的行为
test("user can checkout with valid cart", async () => {
  const cart = createCart();
  cart.add(product);
  const result = await checkout(cart, paymentMethod);
  expect(result.status).toBe("confirmed");
});
```

特征：

- 测用户/调用方关心的行为
- 只用公共 API
- 重构内部活下来
- 描述 WHAT，不描述 HOW
- 一个测试一个逻辑断言

## 坏测试

**实现细节的测试**：耦合到内部结构。

```typescript
// 坏：测实现细节
test("checkout calls paymentService.process", async () => {
  const mockPayment = jest.mock(paymentService);
  await checkout(cart, payment);
  expect(mockPayment.process).toHaveBeenCalledWith(cart.total);
});
```

红旗：

- mock 内部协作者
- 测私有方法
- 断言调用次数/顺序
- 没有行为变化的重构就让测试破
- 测试名描述 HOW 不是 WHAT
- 通过外部手段而不是接口来验证

```typescript
// 坏：绕过接口验证
test("createUser saves to database", async () => {
  await createUser({ name: "Alice" });
  const row = await db.query("SELECT * FROM users WHERE name = ?", ["Alice"]);
  expect(row).toBeDefined();
});

// 好：通过接口验证
test("createUser makes user retrievable", async () => {
  const user = await createUser({ name: "Alice" });
  const retrieved = await getUser(user.id);
  expect(retrieved.name).toBe("Alice");
});
```

**重言式测试**：期望值复述了实现，所以测试通过是构造出来的。

```typescript
// 坏：期望值按代码的方式重算
test("calculateTotal sums line items", () => {
  const items = [{ price: 10 }, { price: 5 }];
  const expected = items.reduce((sum, i) => sum + i.price, 0);
  expect(calculateTotal(items)).toBe(expected);
});

// 好：期望值是独立的、已知的字面量
test("calculateTotal sums line items", () => {
  expect(calculateTotal([{ price: 10 }, { price: 5 }])).toBe(15);
});
```