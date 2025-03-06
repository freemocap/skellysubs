// subtitleOptionsSlice.ts
import { createSlice, PayloadAction } from "@reduxjs/toolkit";
import { RootState } from "../AppStateStore";

export interface SubtitleOption {
    id: string;
    name: string;
    type: "original" | "translated";
    language: string;
    vttContent: string;
}

interface SubtitleOptionsState {
    subtitles: SubtitleOption[];
    selectedId: string | null;
}

const initialState: SubtitleOptionsState = {
    subtitles: [],
    selectedId: null,
};

export const subtitleOptionsSlice = createSlice({
    name: "subtitleOptions",
    initialState,
    reducers: {
        addSubtitle: (state, action: PayloadAction<SubtitleOption>) => {
            const existing = state.subtitles.find(s => s.id === action.payload.id);
            if (!existing) {
                state.subtitles.push(action.payload);
                if (!state.selectedId) state.selectedId = action.payload.id;
            }
        },
        updateSubtitle: (
            state,
            action: PayloadAction<{ id: string; vttContent: string }>
        ) => {
            const subtitle = state.subtitles.find(s => s.id === action.payload.id);
            if (subtitle) subtitle.vttContent = action.payload.vttContent;
        },
        selectSubtitle: (state, action: PayloadAction<string>) => {
            if (state.subtitles.some(s => s.id === action.payload)) {
                state.selectedId = action.payload;
            }
        },
    },
});

export const { addSubtitle, updateSubtitle, selectSubtitle } =
    subtitleOptionsSlice.actions;

export const selectSubtitles = (state: RootState) =>
    state.subtitleOptions.subtitles;
export const selectSelectedSubtitle = (state: RootState) =>
    state.subtitleOptions.subtitles.find(
        s => s.id === state.subtitleOptions.selectedId
    );

export default subtitleOptionsSlice.reducer;