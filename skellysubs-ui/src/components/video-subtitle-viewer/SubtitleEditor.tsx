import {useEffect, useRef, useState} from "react";
import { Editor } from "@monaco-editor/react";
import type {Subtitle} from "./video-subtitle-viewer-types";

interface SubtitleEditorProps {
    vttContent: string;
    currentSubtitle?: Subtitle | null;
    parsedSubtitles: Subtitle[]
    onContentChange: (value: string) => void;
}

export const SubtitleEditor = ({
                                   vttContent,
                                   currentSubtitle,
                                   parsedSubtitles,
                                   onContentChange
                               }: SubtitleEditorProps) => {
    const editorRef = useRef<any>(null);
    const monacoRef = useRef<any>(null);
    const [activeDecoration, setActiveDecoration] = useState<string[]>([])

    const handleEditorDidMount = (editor: any, monaco: any) => {
        editorRef.current = editor;
        monacoRef.current = monaco;
    };

    useEffect(() => {
        const editor = editorRef.current
        if (!editor || !currentSubtitle) return

        const cueIndex = parsedSubtitles.findIndex(
            sub => sub.start === currentSubtitle.start && sub.end === currentSubtitle.end
        )

        if (cueIndex === -1) return

        const blocks = vttContent.split(/\n\n+/g)
        let lineNumber = 0
        for (let i = 0; i < cueIndex; i++) {
            lineNumber += blocks[i].split('\n').length + 1
        }

        const newDecoration = editor.deltaDecorations(activeDecoration, [
            {
                range: new monacoRef.current.Range(lineNumber + 1, 1, lineNumber + blocks[cueIndex].split('\n').length + 1, 1),
                options: { className: 'active-subtitle-line' }
            }
        ])
        setActiveDecoration(newDecoration)
    }, [currentSubtitle, vttContent])
    return (
        <Editor
            height="100%"
            language="plaintext"
            theme="vs-dark"
            value={vttContent}
            onChange={(value) => value && onContentChange(value)}
            onMount={handleEditorDidMount}
            options={{
                minimap: { enabled: false },
                lineNumbers: "on",
                scrollBeyondLastLine: false,
                wordWrap: "on",
            }}
        />
    );
};
