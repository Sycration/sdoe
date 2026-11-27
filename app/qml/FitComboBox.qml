import QtQuick
import QtQuick.Controls

// ComboBox as wide as its widest entry, with a list at least that wide when
// a layout sets a smaller width. Styles such as Fusion size the box for the
// current entry only and make the list as wide as the box, which cuts off
// longer entries.
ComboBox {
    id: control
    implicitContentWidthPolicy: ComboBox.WidestText
    FontMetrics { id: metrics; font: control.font }
    popup.width: {
        let w = 0
        for (let i = 0; i < count; i++) w = Math.max(w, metrics.advanceWidth(textAt(i)))
        return Math.max(width, Math.ceil(w) + leftPadding + rightPadding)
    }
}
