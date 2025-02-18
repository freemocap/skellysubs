// src/features/websocket/websocketSlice.ts
import type { PayloadAction } from "@reduxjs/toolkit"
import { createSlice } from "@reduxjs/toolkit"

interface WebSocketState {
  dataBySessionId: Record<string, any>
}

const initialState: WebSocketState = {
  dataBySessionId: {},
}

export interface UpdateDataPayload {
  sessionId: string
  data: any
}

export const websocketSlice = createSlice({
  name: "websocket",
  initialState,
  reducers: {
    updateSessionData: (state, action: PayloadAction<UpdateDataPayload>) => {
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

export const { updateSessionData } = websocketSlice.actions
