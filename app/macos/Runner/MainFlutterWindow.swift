import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    self.contentViewController = flutterViewController

    // 透明标题栏，内容延伸到窗口顶部：导航栏一直通到红绿灯按钮下面。
    self.titlebarAppearsTransparent = true
    self.titleVisibility = .hidden
    self.styleMask.insert(.fullSizeContentView)

    // 最小可以缩到手机大小（iPhone SE 的 375×667），窄时界面换成底部标签栏。
    self.contentMinSize = NSSize(width: 375, height: 667)
    self.setContentSize(NSSize(width: 1160, height: 780))
    self.center()

    RegisterGeneratedPlugins(registry: flutterViewController)

    super.awakeFromNib()
  }
}
