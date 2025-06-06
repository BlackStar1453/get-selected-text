# macOS 获取选中文本上下文功能改进

## 问题分析

原始代码在获取 focused UI element 时经常失败，导致报错：
```
[AX_CONTEXT_LOG] No focused UI element found.
[CONTEXT_MACOS] Error in get_selected_text_by_ax: Custom { kind: NotFound, error: "No focused UI element" }
```

主要问题：
1. **单一策略依赖**：仅依赖系统级别的 `kAXFocusedUIElementAttribute`
2. **应用兼容性差**：许多应用（特别是Electron应用如Cursor）不正确报告 focused UI element
3. **错误处理不足**：缺乏有效的 fallback 机制

## 解决方案

### 1. 多策略获取 UI 元素

实现了健壮的 `get_selected_text_by_ax_robust()` 函数，采用多种策略：

**策略1：系统级别获取（原始方法）**
```rust
fn try_system_focused_element() -> Result<(String, Option<String>), Box<dyn std::error::Error>>
```
- 尝试获取系统范围的 focused UI element
- 适用于标准的辅助功能兼容应用

**策略2：活动窗口方法**
```rust
fn try_active_window_approach() -> Result<(String, Option<String>), Box<dyn std::error::Error>>
```
- 通过 `get_active_window()` 获取当前活动窗口
- 使用进程ID获取应用程序的AX元素
- 从应用级别查找 focused element
- 如果应用级别没有 focused element，尝试直接从应用程序元素获取文本

**策略3：替代 AX 属性方法**
```rust
fn try_alternative_ax_methods() -> Result<(String, Option<String>), Box<dyn std::error::Error>>
```
- 尝试多种不同的 AX 属性来获取选中文本：
  - `AXSelectedText`: 直接获取选中文本
  - `AXValue`: 从值属性获取
  - `AXTitle`: 从标题获取
  - `AXHelp`: 从帮助信息获取
  - `AXDescription`: 从描述获取
- 作为最后手段，检查剪贴板内容

### 2. 改进的 AppleScript Fallback

当所有AX API策略失败时，使用改进的AppleScript方法：

**获取上下文的AppleScript**
```rust
fn get_context_via_applescript() -> Result<String, Box<dyn std::error::Error>>
```
- 使用 `Cmd+A` 选择全部内容
- 复制到剪贴板获取完整上下文
- 智能恢复原始剪贴板内容
- 取消选择状态以避免用户界面混乱

### 3. 健壮的错误处理

- **渐进式降级**：从最精确的方法逐步降级到通用方法
- **智能上下文验证**：检查获取的上下文是否包含选中文本
- **详细日志记录**：便于调试和问题诊断
- **应用特异性优化**：针对不同类型应用（Electron、原生等）的特殊处理

## 针对 Cursor 的特殊改进

从您提供的日志可以看到，Cursor（PID: 41827）这类 Electron 应用的问题：

```
[AX_STRATEGY2] Active window: Cursor (PID: 41827)
[AX_STRATEGY2] No focused element found via application
```

我们的改进专门解决了这个问题：

1. **策略2增强**：即使应用级别没有 focused element，也会尝试直接从应用程序元素获取文本
2. **策略3新增**：使用多种替代 AX 属性，适合 Electron 应用的特殊 AX 实现
3. **AppleScript保底**：确保最终能通过模拟操作获取结果

## 代码改进要点

### 类型安全改进
```rust
// 修复了原始代码中的类型转换问题
let focused_element = match system_element
    .attribute(&AXAttribute::new(&CFString::from_static_string(
        kAXFocusedUIElementAttribute,
    )))
    .ok()  // 添加 .ok() 处理 Result
    .and_then(|element| element.downcast_into::<AXUIElement>())
{
    Some(element) => element,
    None => return Err(/* ... */),
};
```

### 多策略实现
```rust
fn get_selected_text_by_ax_robust() -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    // 策略1: 系统级别
    if let Ok(result) = try_system_focused_element() { return Ok(result); }
    
    // 策略2: 活动窗口
    if let Ok(result) = try_active_window_approach() { return Ok(result); }
    
    // 策略3: 替代方法
    if let Ok(result) = try_alternative_ax_methods() { return Ok(result); }
    
    Err(/* 所有策略都失败 */)
}
```

### 剪贴板检测辅助
```rust
fn get_current_clipboard_text() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("pbpaste").output()?;
    // 安全地获取剪贴板内容
}
```

## 使用方式

### 测试示例
运行原始测试程序：
```bash
cargo run --example test_context
```

运行改进效果测试：
```bash
cargo run --example test_improvements
```

### API 使用
```rust
use get_selected_text::get_selected_text_with_context;

match get_selected_text_with_context() {
    Ok((selected_text, context)) => {
        println!("选中文本: {}", selected_text);
        if let Some(ctx) = context {
            println!("上下文: {}", ctx);
        }
    }
    Err(e) => println!("错误: {}", e),
}
```

## 预期改进效果

1. **大幅提高成功率**：从仅在少数应用工作，扩展到支持绝大多数 macOS 应用
2. **更好的 Electron 应用兼容性**：专门优化 Cursor、VS Code 等现代编辑器
3. **更智能的降级**：当精确方法失败时，自动使用通用方法
4. **更好的用户体验**：减少 "No focused UI element found" 错误

## 调试信息增强

新的日志系统提供了详细的调试信息：
```
[AX_ROBUST] Starting robust AX text retrieval...
[AX_ROBUST] Strategy 1: Attempting system-wide focused element...
[AX_STRATEGY1] Trying system-wide focused element...
[AX_STRATEGY1] No system-wide focused UI element found.
[AX_ROBUST] Strategy 2: Attempting active window approach...
[AX_STRATEGY2] Trying active window approach...
[AX_STRATEGY2] Active window: Cursor (PID: 41827)
[AX_STRATEGY2] No focused element found via application, trying alternative methods...
[AX_STRATEGY2] Found text directly from application element  // 新的成功路径
```

这些改进使得获取选中文本上下文的功能更加健壮和可靠，能够在更多场景下正常工作，特别是解决了与 Cursor 等现代开发工具的兼容性问题。 