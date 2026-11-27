import QtQuick
import QtQuick.Controls

// Menu as wide as its widest entry. Styles such as Fusion (the style of the
// WebAssembly build) give menus a fixed width and cut off longer entries.
Menu {
    width: {
        let w = 0
        for (let i = 0; i < count; i++) {
            const item = itemAt(i)
            if (item) w = Math.max(w, item.implicitWidth)
        }
        return Math.max(implicitWidth, w + leftPadding + rightPadding)
    }
}
