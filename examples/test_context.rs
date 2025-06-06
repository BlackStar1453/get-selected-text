use get_selected_text::get_selected_text_with_context;

fn main() {
    println!("=== 测试获取选中文本和上下文功能 ===");
    println!("请在任何应用中选中一些文本，然后按Enter继续...");
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    
    println!("正在获取选中文本和上下文...");
    
    match get_selected_text_with_context() {
        Ok((selected_text, context)) => {
            println!("\n✅ 成功获取结果:");
            println!("选中文本: \"{}\"", selected_text);
            match context {
                Some(ctx) => {
                    println!("上下文长度: {} 字符", ctx.len());
                    println!("上下文内容: \"{}\"", ctx);
                    
                    // 检查选中文本是否在上下文中
                    if ctx.contains(&selected_text) {
                        println!("✅ 选中文本在上下文中找到");
                    } else {
                        println!("⚠️ 选中文本未在上下文中找到");
                    }
                }
                None => {
                    println!("上下文: 无");
                }
            }
        }
        Err(e) => {
            println!("❌ 错误: {}", e);
        }
    }
    
    println!("\n=== 测试完成 ===");
} 