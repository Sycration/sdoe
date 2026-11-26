import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Sdoe

// Form generated from engine option specs [{key, label, kind, default, min, max, choices, tooltip}].
ColumnLayout {
    id: form
    property var specs: []
    property var values: ({})
    property real labelWidth: 190
    signal edited()
    spacing: 6

    onSpecsChanged: {
        const v = {}
        for (const s of specs) v[s.key] = (values && values[s.key] !== undefined) ? values[s.key] : s.default
        values = v
    }

    function setValue(k, v) {
        values[k] = v
        edited()
    }

    function collect() {
        const o = {}
        for (const s of specs) o[s.key] = values[s.key] !== undefined ? values[s.key] : s.default
        return o
    }

    Repeater {
        model: form.specs
        delegate: RowLayout {
            id: row
            required property var modelData
            Layout.fillWidth: true
            spacing: 8
            Label {
                visible: row.modelData.kind !== "bool"
                text: row.modelData.label
                Layout.preferredWidth: form.labelWidth
                elide: Text.ElideRight
                ToolTip.visible: lhover.hovered && row.modelData.tooltip.length > 0
                ToolTip.text: row.modelData.tooltip
                ToolTip.delay: 500
                HoverHandler { id: lhover }
            }
            Loader {
                Layout.fillWidth: true
                sourceComponent: {
                    switch (row.modelData.kind) {
                    case "int": return intEditor
                    case "float": return textEditor
                    case "bool": return boolEditor
                    case "choice": return choiceEditor
                    default: return textEditor
                    }
                }
                Component {
                    id: intEditor
                    SpinBox {
                        from: row.modelData.min !== undefined ? row.modelData.min : 0
                        to: row.modelData.max !== undefined ? row.modelData.max : 100000
                        editable: true
                        value: Number(form.values[row.modelData.key])
                        onValueModified: form.setValue(row.modelData.key, value)
                    }
                }
                Component {
                    id: textEditor
                    TextField {
                        text: String(form.values[row.modelData.key] !== undefined ? form.values[row.modelData.key] : "")
                        placeholderText: row.modelData.tooltip
                        onTextEdited: form.setValue(row.modelData.key, row.modelData.kind === "float" ? text : text)
                        ToolTip.visible: hovered && row.modelData.tooltip.length > 0
                        ToolTip.text: row.modelData.tooltip
                        ToolTip.delay: 500
                    }
                }
                Component {
                    id: boolEditor
                    CheckBox {
                        text: row.modelData.label
                        checked: form.values[row.modelData.key] === true
                        onToggled: form.setValue(row.modelData.key, checked)
                        ToolTip.visible: hovered && row.modelData.tooltip.length > 0
                        ToolTip.text: row.modelData.tooltip
                        ToolTip.delay: 500
                    }
                }
                Component {
                    id: choiceEditor
                    ComboBox {
                        model: row.modelData.choices.map(c => c[1])
                        currentIndex: Math.max(0, row.modelData.choices.findIndex(c => c[0] === String(form.values[row.modelData.key])))
                        onActivated: (i) => form.setValue(row.modelData.key, row.modelData.choices[i][0])
                    }
                }
            }
        }
    }
}
