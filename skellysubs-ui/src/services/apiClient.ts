// services/api.ts
import axios from "axios"
import { getApiBaseUrl } from "../utils/getApiBaseUrl"

const apiClient = axios.create({
  baseURL: getApiBaseUrl(),
  timeout: 10000,
})

export const ProcessingAPI = {
  // replace with real stuff
  //
  // uploadFile: (file: File) => {
  //     const formData = new FormData()
  //     formData.append('file', file)
  //     return apiClient.post('/upload', formData)
  // },
  // scrapeAudio: (videoPath: string) => {
  //     return apiClient.post('/scrape-audio', { videoPath })
  // }
}
