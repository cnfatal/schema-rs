# schema-rs-win32

使用 Win32 原生控件渲染 JSON Schema 表单的 crate。传入 JSON Schema 和初始值，弹出原生 Windows 窗口，用户填写后关闭窗口返回最终 JSON 值。

## 快速使用

```rust
use schema_rs_core::{DefaultValidator, SchemaRuntime};
use schema_rs_win32::run_schema_form;

let schema = serde_json::json!({
    "title": "用户信息",
    "type": "object",
    "required": ["name"],
    "properties": {
        "name": { "type": "string", "title": "姓名" },
        "age":  { "type": "integer", "title": "年龄" },
        "bio":  { "type": "string", "title": "简介", "format": "textarea" }
    }
});

let runtime = SchemaRuntime::new(
    Box::new(DefaultValidator::new()),
    schema,
    serde_json::json!({}),
);

// 阻塞，窗口关闭后返回用户填写的值
let result = run_schema_form("用户信息", runtime);
println!("{}", result);
```

## Schema 配置说明

### 基础字段

| 字段          | 作用                                                                  |
| ------------- | --------------------------------------------------------------------- |
| `title`       | 控件标签文本。缺失时使用属性名，根节点显示 `"Root"`。同时作为窗口标题 |
| `description` | 在标签和控件之间显示描述文本                                          |
| `required`    | 父对象的 `required` 数组中的字段，标签末尾显示 ` *` 标记              |
| `readOnly`    | `true` 时控件只读（文本框不可编辑，复选框禁用）                       |
| `const`       | 存在时等同于 `readOnly: true`，控件只读                               |
| `enum`        | 字符串类型有 `enum` 时渲染为下拉列表（ComboBox）                      |
| `default`     | 初始默认值（由 core 层自动应用）                                      |

### 类型与控件映射

| Schema `type` | 渲染控件                                                  |
| ------------- | --------------------------------------------------------- |
| `string`      | 文本框（默认）、下拉列表（有 `enum`）、或按 `format` 分派 |
| `number`      | 数字文本框                                                |
| `integer`     | 整数文本框                                                |
| `boolean`     | 复选框（CheckBox）                                        |
| `object`      | 容器，递归渲染子属性                                      |
| `array`       | 容器，递归渲染子元素 + "Add Item" 按钮                    |

### `format` 扩展

字符串类型支持以下 `format` 值来改变控件类型：

| `format`           | 控件           | 说明                                              |
| ------------------ | -------------- | ------------------------------------------------- |
| `"textarea"`       | 多行文本框     | 4 行高度，支持换行和垂直滚动                      |
| `"password"`       | 密码框         | 输入内容以圆点遮蔽                                |
| `"file-path"`      | 文件路径选择器 | 文本框 + "Browse…" 按钮，弹出系统文件选择对话框   |
| `"directory-path"` | 目录路径选择器 | 文本框 + "Browse…" 按钮，弹出系统文件夹选择对话框 |
| 其他               | 普通文本框     | `"uri"`、`"email"`、`"date"` 等标准值不做特殊处理 |

### `x-*` 扩展字段

通过 schema 的扩展字段控制布局：

#### `x-layout`

| 父类型   | `x-layout` 值 | 效果                                                  |
| -------- | ------------- | ----------------------------------------------------- |
| `object` | `"tabs"`      | 将子属性渲染为标签页（Tab Control），每个属性一个 tab |
| `array`  | `"table"`     | 将子元素渲染为表格，列从第一行对象的属性推导          |

**Tab 布局示例：**

```json
{
  "type": "object",
  "x-layout": "tabs",
  "properties": {
    "basic": {
      "title": "基本信息",
      "type": "object",
      "properties": {
        "name": { "type": "string", "title": "姓名" }
      }
    },
    "advanced": {
      "title": "高级设置",
      "type": "object",
      "properties": {
        "debug": { "type": "boolean", "title": "调试模式" }
      }
    }
  }
}
```

**表格布局示例：**

```json
{
  "type": "array",
  "title": "用户列表",
  "x-layout": "table",
  "items": {
    "type": "object",
    "properties": {
      "name": { "type": "string", "title": "姓名" },
      "age": { "type": "integer", "title": "年龄" },
      "active": { "type": "boolean", "title": "启用" }
    }
  }
}
```

表格中每列支持的控件类型包括文本框、下拉列表、数字框、复选框，末列自动添加删除按钮。

#### `x-order`

控制对象属性的显示顺序（数值越小越靠前）。由 core 层处理，win32 层按排序后的顺序渲染。

```json
{
  "type": "object",
  "properties": {
    "email": { "type": "string", "x-order": 2 },
    "name": { "type": "string", "x-order": 1 }
  }
}
```

上例中 `name` 排在 `email` 前面。

### 动态操作

| 场景         | UI 元素             | 条件                                          |
| ------------ | ------------------- | --------------------------------------------- |
| 对象添加属性 | "Add Property" 按钮 | `can_add = true`（有 `additionalProperties`） |
| 数组添加元素 | "Add Item" 按钮     | `can_add = true`                              |
| 删除可选字段 | "Remove" 按钮       | `can_remove = true`（非 required 字段）       |
| 表格删除行   | "✕" 按钮            | 表格布局中每行末列                            |

### 验证错误

字段验证失败时，在控件下方显示 `⚠ {错误类型}` 提示文本。验证由 core 层在值变更时自动触发。
