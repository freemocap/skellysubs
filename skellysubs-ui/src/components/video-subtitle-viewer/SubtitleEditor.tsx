import { useRef } from "react";
import { Editor } from "@monaco-editor/react";
import {Subtitle} from "./video-subtitle-viewer-types";

interface SubtitleEditorProps {
    vttContent: string;
    currentSubtitle?: Subtitle | null;
    onContentChange: (value: string) => void;
}

export const SubtitleEditor = ({
                                   vttContent,
                                   currentSubtitle,
                                   onContentChange
                               }: SubtitleEditorProps) => {
    const editorRef = useRef<any>(null);
    const monacoRef = useRef<any>(null);

    const handleEditorDidMount = (editor: any, monaco: any) => {
        editorRef.current = editor;
        monacoRef.current = monaco;
    };

    return (
        <Editor
            height="100%"
            language="plaintext"
            theme="vs-dark"
            value={vttContent}
            onChange={(value: string) => value && onContentChange(value)}
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