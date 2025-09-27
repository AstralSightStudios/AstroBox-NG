# AstroBox Custom Plugins
`btclassic-spp`: 适用于Windows / macOS / Linux / Android的经典蓝牙(SPP)接口绑定实现
`live-activity`: 适用于Windows / macOS / iOS的实时活动接口绑定实现
## 疑难杂症 & 报错解决
### No matching configuration of project :tauri-android was found.
解决方案：新建`插件名/android/.tauri/tauri-api`文件夹，克隆[https://github.com/tauri-apps/tauri](https://github.com/tauri-apps/tauri)存储库，将`crates/tauri/mobile/android`里面的所有内容复制到刚刚新建的文件夹中，Rebuild即可

### Xcode报错Swift Package找不到包Tauri
解决方案：新疆`插件名/.tauri/tauri-api`文件夹，克隆[https://github.com/tauri-apps/tauri](https://github.com/tauri-apps/tauri)存储库，将`crates/tauri/mobile/ios-api`里面的所有内容复制到刚刚新建的文件夹中，Rebuild即可

### Xcode报错ABWidgetsLiveActivity No such module 'live_activity'
Xcode左侧边栏中右键，Add Package Dependencies，Add Local，定位到`src-tauri/plugins/live-activity/ios`，添加，遇到报错直接选择Add Anyway，然后在“Choose Package Products for ios”页面中将live-activity这个Library的Add to Target设置为ABWidgetsExtension

**注意：不要把你修复问题产生的狗屎提交到仓库。**