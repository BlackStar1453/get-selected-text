use get_selected_text::get_selected_text_with_context;

fn main() {
    println!("=== 测试改进后的多策略获取方法 ===");
    println!("这个测试程序将显示详细的调试信息，帮助了解哪个策略成功了");
    println!();
    
    // 提示用户操作
    println!("请按以下步骤测试：");
    println!("1. 在 Cursor、VS Code、Chrome 或其他应用中选中一些文本");
    println!("2. 回到此终端窗口按 Enter 开始测试");
    println!("3. 观察调试日志，了解哪个策略起作用");
    println!();
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    
    println!("🔍 开始获取选中文本和上下文...");
    println!("📝 调试日志如下：");
    println!("{:=<60}", "");
    
    let start_time = std::time::Instant::now();
    
    match get_selected_text_with_context() {
        Ok((selected_text, context)) => {
            let duration = start_time.elapsed();
            
            println!("{:=<60}", "");
            println!("✅ 成功获取结果！耗时: {:?}", duration);
            println!();
            
            println!("📄 选中文本:");
            println!("   长度: {} 字符", selected_text.len());
            println!("   内容: \"{}\"", selected_text);
            println!();
            
            match context {
                Some(ctx) => {
                    println!("📖 上下文:");
                    println!("   长度: {} 字符", ctx.len());
                    println!("   内容: \"{}\"", if ctx.len() > 200 { 
                        format!("{}...", &ctx[..200]) 
                    } else { 
                        ctx.clone() 
                    });
                    println!();
                    
                    // 验证选中文本是否在上下文中
                    if ctx.contains(&selected_text) {
                        println!("✅ 验证通过：选中文本在上下文中找到");
                        
                        // 计算选中文本在上下文中的位置
                        if let Some(pos) = ctx.find(&selected_text) {
                            let context_before = &ctx[..pos];
                            let context_after = &ctx[pos + selected_text.len()..];
                            
                            println!("📍 位置信息:");
                            println!("   前文: \"{}\"", if context_before.len() > 50 { 
                                format!("...{}", &context_before[context_before.len()-50..]) 
                            } else { 
                                context_before.to_string() 
                            });
                            println!("   后文: \"{}\"", if context_after.len() > 50 { 
                                format!("{}...", &context_after[..50.min(context_after.len())]) 
                            } else { 
                                context_after.to_string() 
                            });
                        }
                    } else {
                        println!("⚠️ 警告：选中文本未在上下文中找到");
                        println!("   这可能表示获取的上下文不准确");
                    }
                }
                None => {
                    println!("📖 上下文: 无");
                    println!("   注意：成功获取选中文本，但无法获取上下文");
                }
            }
        }
        Err(e) => {
            let duration = start_time.elapsed();
            println!("{:=<60}", "");
            println!("❌ 获取失败！耗时: {:?}", duration);
            println!("错误信息: {}", e);
            println!();
            println!("💡 建议:");
            println!("1. 确保已选中文本");
            println!("2. 检查应用程序的辅助功能权限");
            println!("3. 尝试在不同的应用中测试（如备忘录、TextEdit）");
        }
    }
    
    println!();
    println!("=== 测试完成 ===");
    println!("💡 提示：如果某个策略总是失败，请记录具体应用名称以便进一步优化");
} 