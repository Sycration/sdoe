import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Heading followed by content.
ColumnLayout {
    id: root
    property alias title: heading.text
    default property alias content: body.data
    spacing: 4
    Label {
        id: heading
        font.bold: true
        visible: text.length > 0
        Layout.topMargin: 6
    }
    ColumnLayout {
        id: body
        Layout.fillWidth: true
        Layout.fillHeight: true
        spacing: 4
    }
}
