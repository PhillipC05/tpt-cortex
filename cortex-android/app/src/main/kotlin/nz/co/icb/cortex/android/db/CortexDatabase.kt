package nz.co.icb.cortex.android.db

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase

@Database(entities = [RecordEntity::class], version = 1, exportSchema = false)
abstract class CortexDatabase : RoomDatabase() {

    abstract fun recordDao(): RecordDao

    companion object {
        @Volatile
        private var INSTANCE: CortexDatabase? = null

        fun getInstance(context: Context): CortexDatabase {
            return INSTANCE ?: synchronized(this) {
                INSTANCE ?: Room.databaseBuilder(
                    context.applicationContext,
                    CortexDatabase::class.java,
                    "cortex_database"
                ).build().also { INSTANCE = it }
            }
        }
    }
}
