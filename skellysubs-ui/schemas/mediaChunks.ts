import { z } from "zod"

export const VideoChunkSchema = z.object({
  id: z.string(),
  timestamp: z.number(),
  videoBlob: z.instanceof(Blob),
})

export const AudioChunkSchema = z.object({
  id: z.string(),
  timestamp: z.number(),
  audioBlob: z.instanceof(Blob),
})

export type VideoChunk = z.infer<typeof VideoChunkSchema>
export type AudioChunk = z.infer<typeof AudioChunkSchema>

// Helper functions to create new chunks
export const createVideoChunk = (videoBlob: Blob): VideoChunk => ({
  id: Math.random().toString(36).substr(2, 9),
  timestamp: Date.now(),
  videoBlob,
})

export const createAudioChunk = (audioBlob: Blob): AudioChunk => ({
  id: Math.random().toString(36).substr(2, 9),
  timestamp: Date.now(),
  audioBlob,
})

