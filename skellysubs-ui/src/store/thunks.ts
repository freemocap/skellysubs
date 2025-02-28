import { getApiBaseUrl } from "../utils/getApiBaseUrl"
import { createAsyncThunk } from "@reduxjs/toolkit"
import { ffmpegService } from "../services/FfmpegService/useFfmpeg"
import type {
  ProcessingContext,
  ProcessingState,
} from "./slices/processingStatusSlice"

// Generic stage thunk creator
function createProcessingThunk<InputType, OutputType>(
  stageName: string,
  processor: (
    context: ProcessingContext,
    input?: InputType,
  ) => Promise<OutputType>,
) {
  return createAsyncThunk(
    `processing/${stageName}`,
    async (input: InputType, { getState, rejectWithValue }) => {
      const state = getState() as { processing: ProcessingState }
      const stage = state.processing.stages[stageName]

      // Check requirements
      const missingRequirements = stage.requirements.filter(
        req => !state.processing.context[req],
      )

      if (missingRequirements.length > 0) {
        return rejectWithValue(
          `Missing required data: ${missingRequirements.join(", ")}`,
        )
      }

      try {
        return await processor(state.processing.context, input)
      } catch (error) {
        return rejectWithValue(
          error instanceof Error ? error.message : "Unknown error",
        )
      }
    },
  )
}

// Example thunk for file preparation
export const prepareFileThunk = createProcessingThunk<
  File,
  ProcessingContext["mp3Audio"]
>("filePreparation", async (context: ProcessingContext, file?: File) => {
  if (!file) throw new Error("No file provided")
  if (!ffmpegService.isLoaded) await ffmpegService.loadFfmpeg()

  const { audioBlob, bitrate, duration } =
    await ffmpegService.convertToMP3(file)
  if (!audioBlob) throw new Error("Audio conversion failed")

  return {
    url: URL.createObjectURL(audioBlob),
    name: file.name,
    type: file.type,
    size: audioBlob.size,
    bitrate,
    duration,
  }
})
export const transcribeAudioThunk = createProcessingThunk<
  void, // No input needed
  ProcessingContext["transcription"]
>("transcription", async context => {
  try {
    if (!context.mp3Audio?.url) throw new Error("No audio URL provided")

    console.log("Fetching MP3 from URL:", context.mp3Audio.url)
    const response = await fetch(context.mp3Audio.url)
    if (!response.ok) throw new Error("Failed to fetch MP3")

    const mp3Blob = await response.blob()
    console.log("MP3 Blob fetched successfully -- size:", mp3Blob.size)

    const formData = new FormData()
    formData.append("audio_file", mp3Blob, "audio.mp3")
    const transcribeEndpointUrl = `${getApiBaseUrl()}/processing/transcribe`
    console.log(
      `Sending formData to url: ${transcribeEndpointUrl} -- formData:`,
      formData,
    )
    const transcribeResponse = await fetch(transcribeEndpointUrl, {
      method: "POST",
      body: formData,
    })

    if (!transcribeResponse.ok) {
      throw new Error(`HTTP error ${transcribeResponse.status}`)
    }

    const result = await transcribeResponse.json()
    console.log(`Transcription result: ${JSON.stringify(result, null, 2)}`)
    return result as ProcessingContext["transcription"]
  } catch (error) {
    console.error("Transcription error:", error)
    throw error instanceof Error ? error : new Error("Unknown error")
  }
})
