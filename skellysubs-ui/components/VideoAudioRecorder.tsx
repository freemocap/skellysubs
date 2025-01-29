"use client"

import React, { useRef, useState, useEffect } from "react"
import { Button } from "@/components/ui/button"
import { type VideoChunk, type AudioChunk, createVideoChunk, createAudioChunk } from "../schemas/mediaChunks"
import Editor, { loader } from "@monaco-editor/react"
import ReactMarkdown from "react-markdown"

// Import Monaco Editor themes
import * as monaco from "monaco-editor"
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker"
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker"
import cssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker"
import htmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker"
import tsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker"

// Configure Monaco Editor workers
self.MonacoEnvironment = {
  getWorker(_, label) {
    if (label === "json") {
      return new jsonWorker()
    }
    if (label === "css" || label === "scss" || label === "less") {
      return new cssWorker()
    }
    if (label === "html" || label === "handlebars" || label === "razor") {
      return new htmlWorker()
    }
    if (label === "typescript" || label === "javascript") {
      return new tsWorker()
    }
    return new editorWorker()
  },
}

loader.config({ monaco })

const CHUNK_DURATION = 10000 // 10 seconds

export default function VideoAudioRecorder() {
  const videoRef = useRef<HTMLVideoElement>(null)
  const videoRecorderRef = useRef<MediaRecorder | null>(null)
  const audioRecorderRef = useRef<MediaRecorder | null>(null)
  const [isRecording, setIsRecording] = useState(false)
  const [videoChunks, setVideoChunks] = useState<VideoChunk[]>([])
  const [audioChunks, setAudioChunks] = useState<AudioChunk[]>([])
  const [notes, setNotes] = useState(
    '# Notes\n\nEnter your markdown notes here...\n\n## Bullet Points\n\n- Item 1\n- Item 2\n- Item 3\n\n## Code Block\n\n```json\n{\n  "key": "value",\n  "array": [1, 2, 3]\n}\n```',
  )
  const [isPreview, setIsPreview] = useState(false)
  const [editorLanguage, setEditorLanguage] = useState<"markdown" | "json">("markdown")
  const [currentChunkNumber, setCurrentChunkNumber] = useState(0)
  const [currentTimestamp, setCurrentTimestamp] = useState(0)

  useEffect(() => {
    let interval: NodeJS.Timeout
    if (isRecording) {
      interval = setInterval(() => {
        setCurrentTimestamp(Date.now())
      }, 1000)
    }
    return () => clearInterval(interval)
  }, [isRecording])

  const startRecording = async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ video: true, audio: true })
      if (videoRef.current) {
        videoRef.current.srcObject = stream
      }

      const videoTrack = stream.getVideoTracks()[0]
      const audioTrack = stream.getAudioTracks()[0]

      const videoStream = new MediaStream([videoTrack])
      const audioStream = new MediaStream([audioTrack])

      const videoRecorder = new MediaRecorder(videoStream)
      const audioRecorder = new MediaRecorder(audioStream)

      videoRecorderRef.current = videoRecorder
      audioRecorderRef.current = audioRecorder

      videoRecorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          const videoChunk = createVideoChunk(event.data)
          setVideoChunks((prev) => [...prev, videoChunk])
          sendChunkToServer(videoChunk, "video")
          setCurrentChunkNumber((prev) => prev + 1)
        }
      }

      audioRecorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          const audioChunk = createAudioChunk(event.data)
          setAudioChunks((prev) => [...prev, audioChunk])
          sendChunkToServer(audioChunk, "audio")
        }
      }

      videoRecorder.start(CHUNK_DURATION)
      audioRecorder.start(CHUNK_DURATION)
      setIsRecording(true)
      setCurrentChunkNumber(0)
      setCurrentTimestamp(Date.now())
    } catch (error) {
      console.error("Error accessing media devices:", error)
    }
  }

  const stopRecording = () => {
    if (videoRecorderRef.current && audioRecorderRef.current) {
      videoRecorderRef.current.stop()
      audioRecorderRef.current.stop()
      setIsRecording(false)
    }
  }

  const sendChunkToServer = (chunk: VideoChunk | AudioChunk, type: "video" | "audio") => {
    // Here you would implement the WebSocket logic to send the chunk to the server
    // For example:
    // socket.send(JSON.stringify({ type, chunk }))
    console.log(`Sending ${type} chunk to server:`, chunk)
  }

  const handleEditorChange = (value: string | undefined) => {
    if (value !== undefined) {
      setNotes(value)
    }
  }

  const togglePreview = () => {
    setIsPreview(!isPreview)
  }

  const toggleLanguage = () => {
    setEditorLanguage((prev) => (prev === "markdown" ? "json" : "markdown"))
  }

  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp)
    return date.toISOString().substr(11, 8) // Returns HH:MM:SS
  }

  return (
    <div className="flex flex-col items-center space-y-4 w-full max-w-6xl">
      <div className="flex w-full gap-4">
        <div className="flex-1 relative">
          <video ref={videoRef} autoPlay muted className="w-full border rounded-lg" />
          {isRecording && (
            <div className="absolute top-0 left-0 bg-black bg-opacity-50 text-white p-2 rounded-br-lg">
              <p>Chunk: {currentChunkNumber}</p>
              <p>Time: {formatTimestamp(currentTimestamp)}</p>
            </div>
          )}
          <div className="mt-4">
            <Button onClick={isRecording ? stopRecording : startRecording}>
              {isRecording ? "Stop Recording" : "Start Recording"}
            </Button>
            <p className="mt-2">Recorded video chunks: {videoChunks.length}</p>
            <p>Recorded audio chunks: {audioChunks.length}</p>
          </div>
        </div>
        <div className="flex-1 h-[500px] flex flex-col">
          <div className="flex justify-between mb-2">
            <h2 className="text-xl font-bold">Notes</h2>
            <div className="space-x-2">
              <Button onClick={toggleLanguage}>
                {editorLanguage === "markdown" ? "Switch to JSON" : "Switch to Markdown"}
              </Button>
              <Button onClick={togglePreview}>{isPreview ? "Edit" : "Preview"}</Button>
            </div>
          </div>
          {isPreview ? (
            <div className="flex-1 overflow-auto border rounded p-4 bg-white prose prose-sm max-w-none">
              <ReactMarkdown>{notes}</ReactMarkdown>
            </div>
          ) : (
            <Editor
              height="100%"
              defaultLanguage="markdown"
              language={editorLanguage}
              value={notes}
              onChange={handleEditorChange}
              theme="vs-dark"
              options={{
                minimap: { enabled: false },
                wordWrap: "on",
                lineNumbers: "off",
                folding: false,
                lineDecorationsWidth: 0,
                lineNumbersMinChars: 0,
                renderIndentGuides: false,
              }}
            />
          )}
        </div>
      </div>
    </div>
  )
}

