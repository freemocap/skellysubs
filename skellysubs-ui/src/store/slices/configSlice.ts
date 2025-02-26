import {createSlice, PayloadAction} from "@reduxjs/toolkit"

type QualityPreset = 'low' | 'medium' | 'high'

interface ConfigState {
    quality: QualityPreset
    targetLanguages: string[]
    maxFileSizeMB: number
}

const initialState: ConfigState = {
    quality: 'medium',
    targetLanguages: ['en'],
    maxFileSizeMB: 25,
}

export const configSlice = createSlice({
    name: "config",
    initialState,
    reducers: {
        setQuality: (state, action: PayloadAction<QualityPreset>) => {
            state.quality = action.payload
        },
        setTargetLanguages: (state, action: PayloadAction<string[]>) => {
            state.targetLanguages = action.payload
        },
    },
})

export const { setQuality, setTargetLanguages } = configSlice.actions
export default configSlice.reducer