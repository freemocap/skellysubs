"use client"

import React, { useRef, useState } from "react"
import { Button } from "@/components/ui/button"
import { Vod } from "../types/Vod"

const CHUNK_DURATION = 10000 // 10 seconds
const OVERLAP_DURATION = 2000 // 2 seconds overlap

export default function VideoRecorder() {
  const videoRef = useRef<HTMLVideoElement>(null)
  const mediaRecorderRef = useRef<MediaRecorder | null>(null)
  const [isRecording, setIsRecording] = useState(false)
  const [recordedChunks, setRecordedChunks] = useState<Vod[]>([])

  const startRecording = async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ video: true, audio: true })
      if (videoRef.current) {
        videoRef.current.srcObject = stream
      }

      const mediaRecorder = new MediaRecorder(stream)
      mediaRecorderRef.current = mediaRecorder

      mediaRecorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          const vod = new Vod(event.data)
          setRecordedChunks((prev) => [...prev, vod])
        }
      }

      mediaRecorder.start()
      setIsRecording(true)

      // Set up overlapping chunks
      const recordChunk = () => {
        if (mediaRecorderRef.current && mediaRecorderRef.current.state === "recording") {
          mediaRecorderRef.current.stop()
          mediaRecorderRef.current.start()
          setTimeout(recordChunk, CHUNK_DURATION - OVERLAP_DURATION)
        }
      }

      setTimeout(recordChunk, CHUNK_DURATION - OVERLAP_DURATION)
    } catch (error) {
      console.error("Error accessing media devices:", error)
    }
  }

  const stopRecording = () => {
    if (mediaRecorderRef.current) {
      mediaRecorderRef.current.stop()
      setIsRecording(false)
    }
  }

  return (
    <div className="flex flex-col items-center space-y-4">
      <video ref={videoRef} autoPlay muted className="w-full max-w-xl border rounded-lg" />
      <Button onClick={isRecording ? stopRecording : startRecording}>
        {isRecording ? "Stop Recording" : "Start Recording"}
      </Button>
      <p>Recorded chunks: {recordedChunks.length}</p>
    </div>
  )
}

