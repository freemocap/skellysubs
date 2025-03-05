// Example thunk for file preparation
import type { ProcessingContext } from "../processing-status-types"
import { ffmpegService } from "../../../../services/FfmpegService/useFfmpeg"
import { createProcessingThunk } from "./createProcessingThunk"

export const prepareFileThunk = createProcessingThunk<
  File,
  ProcessingContext["mp3Audio"]
>("filePreparation", async (context: ProcessingContext, file?: File) => {
  if (!file) throw new Error("No file provided")
  if (!ffmpegService.isLoaded) await ffmpegService.loadFfmpeg()

  const { audioBlob, audioFileName, audioFileType, bitrate, duration } =
    await ffmpegService.convertToMP3(file)
  if (!audioBlob) throw new Error("Audio conversion failed")

  return {
    url: URL.createObjectURL(audioBlob),
    name: audioFileName,
    type: audioFileType,
    size: audioBlob.size,
    bitrate,
    duration,
  }
})
