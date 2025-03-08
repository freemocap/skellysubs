import { useEffect, useRef, useState } from "react"
import { Editor } from "@monaco-editor/react"
import type { Subtitle } from "./video-subtitle-viewer-types"
import {
  defaultFormatting,
  SubtitleFormatting,
  type SubtitleFormatting as SubtitleFormattingType,
} from "./SubtitleFormatting"
import { Box, Tab, Tabs } from "@mui/material"

interface SubtitleEditorProps {
  content: string
  currentSubtitle?: Subtitle | null
  parsedSubtitles: Subtitle[]
  onContentChange: (value: string) => void
}

export const SubtitleEditor = ({
  content,
  currentSubtitle,
  parsedSubtitles,
  onContentChange,
}: SubtitleEditorProps) => {
  const editorRef = useRef<any>(null)
  const monacoRef = useRef<any>(null)
  const [activeDecoration, setActiveDecoration] = useState<string[]>([])
  const [activeTab, setActiveTab] = useState(0)
  const [formatting, setFormatting] =
    useState<SubtitleFormattingType>(defaultFormatting)

  const updateFormatting = <K extends keyof SubtitleFormattingType>(
    key: K,
    value: SubtitleFormattingType[K],
  ) => {
    setFormatting(prev => ({ ...prev, [key]: value }))
  }
  const handleEditorDidMount = (editor: any, monaco: any) => {
    editorRef.current = editor
    monacoRef.current = monaco
  }

  useEffect(() => {
    const editor = editorRef.current
    if (!editor || !currentSubtitle) return

    const cueIndex = parsedSubtitles.findIndex(
      sub =>
        sub.start === currentSubtitle.start && sub.end === currentSubtitle.end,
    )

    if (cueIndex === -1) return

    const blocks = content.split(/\n\n+/g)
    let lineNumber = 0
    for (let i = 0; i < cueIndex; i++) {
      lineNumber += blocks[i].split("\n").length + 1
    }

    const newDecoration = editor.deltaDecorations(activeDecoration, [
      {
        range: new monacoRef.current.Range(
          lineNumber + 1,
          1,
          lineNumber + blocks[cueIndex].split("\n").length + 1,
          1,
        ),
        options: { className: "active-subtitle-line" },
      },
    ])
    setActiveDecoration(newDecoration)
  }, [currentSubtitle, content])

  return (
    <Box sx={{ width: "100%" }}>
      <Tabs
        value={activeTab}
        onChange={(_, newValue) => setActiveTab(newValue)}
      >
        <Tab label="Subtitle Editor" />
        <Tab label="Formatting" />
      </Tabs>

      <Box hidden={activeTab !== 0}>
        <Editor
          height="400px"
          language="plaintext"
          theme="vs-dark"
          value={content}
          onChange={value => value && onContentChange(value)}
          options={{
            minimap: { enabled: false },
            lineNumbers: "on",
            scrollBeyondLastLine: false,
            wordWrap: "on",
          }}
        />
      </Box>

      <Box hidden={activeTab !== 1}>
        <SubtitleFormatting
          formatting={formatting}
          updateFormatting={updateFormatting}
          onReset={() => setFormatting(defaultFormatting)}
        />
      </Box>
    </Box>
  )
}
