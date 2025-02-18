// src/features/websocket/websocketSlice.ts
import type { PayloadAction } from "@reduxjs/toolkit"
import { createSlice } from "@reduxjs/toolkit"

interface WebsocketDataState {
  dataBySessionId: Record<string, any>
}

const initialState: WebsocketDataState = {
  dataBySessionId: {},
}

export interface UpdateWebsocketDataPayload {
  sessionId: string
  data: any
}

export const websocketDataSlice = createSlice({
  name: "websocket",
  initialState,
  reducers: {
    updateSessionData: (
      state,
      action: PayloadAction<UpdateWebsocketDataPayload>,
    ) => {
      if (!state.dataBySessionId[action.payload.sessionId]) {
        state.dataBySessionId[action.payload.sessionId] = []
      }
      state.dataBySessionId[action.payload.sessionId].push(action.payload.data)
      console.log(
        `Updated data for session ${action.payload.sessionId} - length: ${state.dataBySessionId[action.payload.sessionId].length}`,
      )
    },
  },
})

export const { updateSessionData } = websocketDataSlice.actions
